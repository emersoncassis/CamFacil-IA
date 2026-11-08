//! Processos filhos (ngrok, ffmpeg) morrem junto com o CamFácil IA, mesmo
//! quando ele é fechado à força: janela fechada, Gerenciador de Tarefas,
//! desinstalação, queda. Sem dependência nova (FFI direta ao sistema).
//!
//! - Windows: o próprio processo entra num Job Object com
//!   `KILL_ON_JOB_CLOSE`; tudo o que ele abrir herda o job e é encerrado pelo
//!   Windows quando o programa termina. O que deve sobreviver (navegador,
//!   limpeza da desinstalação) sai do job com `CREATE_BREAKAWAY_FROM_JOB`.
//! - Linux/Android: cada filho recebe `PR_SET_PDEATHSIG` (SIGKILL quando o
//!   pai morre).

/// Flag de criação para um processo que deve continuar depois do programa.
#[cfg(windows)]
pub const FORA_DO_JOB: u32 = 0x0100_0000; // CREATE_BREAKAWAY_FROM_JOB

#[cfg(windows)]
mod win {
    use std::ffi::c_void;
    use std::ptr::{null, null_mut};

    type Handle = *mut c_void;

    #[repr(C)]
    struct BasicLimit {
        per_process_user_time_limit: i64,
        per_job_user_time_limit: i64,
        limit_flags: u32,
        minimum_working_set_size: usize,
        maximum_working_set_size: usize,
        active_process_limit: u32,
        affinity: usize,
        priority_class: u32,
        scheduling_class: u32,
    }

    #[repr(C)]
    struct IoCounters {
        valores: [u64; 6],
    }

    #[repr(C)]
    struct ExtendedLimit {
        basic: BasicLimit,
        io: IoCounters,
        process_memory_limit: usize,
        job_memory_limit: usize,
        peak_process_memory_used: usize,
        peak_job_memory_used: usize,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateJobObjectW(atributos: *mut c_void, nome: *const u16) -> Handle;
        fn SetInformationJobObject(
            job: Handle,
            classe: i32,
            info: *mut c_void,
            tamanho: u32,
        ) -> i32;
        fn AssignProcessToJobObject(job: Handle, processo: Handle) -> i32;
        fn GetCurrentProcess() -> Handle;
    }

    const KILL_ON_JOB_CLOSE: u32 = 0x0000_2000;
    const BREAKAWAY_OK: u32 = 0x0000_0800;
    const EXTENDED_LIMIT_INFORMATION: i32 = 9;

    pub fn prender() {
        // SAFETY: chamadas Win32 com ponteiros válidos e estrutura zerada do
        // tamanho exato pedido; o handle do job fica aberto até o processo
        // terminar (é isso que derruba os filhos).
        unsafe {
            let job = CreateJobObjectW(null_mut(), null());
            if job.is_null() {
                return;
            }
            let mut info: ExtendedLimit = std::mem::zeroed();
            info.basic.limit_flags = KILL_ON_JOB_CLOSE | BREAKAWAY_OK;
            let ok = SetInformationJobObject(
                job,
                EXTENDED_LIMIT_INFORMATION,
                &mut info as *mut ExtendedLimit as *mut c_void,
                std::mem::size_of::<ExtendedLimit>() as u32,
            );
            if ok != 0 {
                AssignProcessToJobObject(job, GetCurrentProcess());
            }
        }
    }
}

/// Chamar uma vez, no início do programa.
pub fn iniciar() {
    #[cfg(windows)]
    win::prender();
}

/// Faz o filho morrer junto com o programa (Linux e Android). No Windows o
/// Job Object já cuida disso.
pub fn morrer_com_o_programa(cmd: &mut std::process::Command) {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        use std::os::unix::process::CommandExt;
        extern "C" {
            fn prctl(opcao: i32, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> i32;
        }
        const PR_SET_PDEATHSIG: i32 = 1;
        const SIGKILL: u64 = 9;
        // SAFETY: só chama prctl entre o fork e o exec; não aloca nem trava.
        unsafe {
            cmd.pre_exec(|| {
                prctl(PR_SET_PDEATHSIG, SIGKILL, 0, 0, 0);
                Ok(())
            });
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    let _ = cmd;
}

/// Ao abrir: derruba um ngrok que uma versão anterior (sem o Job Object)
/// deixou rodando. Só o ngrok baixado pelo próprio programa, na pasta de
/// dados; um ngrok do usuário em outro lugar não é tocado.
pub fn limpar_orfaos(dados: &std::path::Path) {
    let bin = dados.join("bin");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let filtro = format!(
            "Get-Process ngrok -ErrorAction SilentlyContinue | Where-Object {{ $_.Path -like '{}\\*' }} | Stop-Process -Force",
            bin.display().to_string().replace('\'', "''")
        );
        let _ = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &filtro])
            .creation_flags(0x0800_0000)
            .status();
    }
    #[cfg(all(unix, not(target_os = "android")))]
    {
        let _ = std::process::Command::new("pkill")
            .args(["-f", &format!("{}/ngrok", bin.display())])
            .status();
    }
    #[cfg(target_os = "android")]
    let _ = bin;
}
