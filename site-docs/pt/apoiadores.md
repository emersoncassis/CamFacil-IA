---
hide:
  - footer
---

# Apoiadores

O CamFácil IA é livre e gratuito, e vai continuar assim. Cada apoio volta todo para o projeto: servidor e site no ar, câmeras e equipamentos para testar, novas inteligências e mais horas de desenvolvimento.

=== "Pix"

    <div class="pix">
      <img class="pix__qr" src="../assets/img/pix-qr.svg" alt="QR code do Pix para apoiar o CamFácil IA" width="200" height="200">
      <div class="pix__dados">
        <p class="pix__rotulo">Chave Pix (CPF)</p>
        <p class="pix__chave">390.690.838-09</p>
        <p class="pix__nome">Emerson de Carvalho Assis</p>
        <div class="pix__acoes">
          <button class="md-button md-button--primary" type="button" data-copiar="39069083809" data-copiado="Chave copiada">Copiar chave</button>
          <button class="md-button" type="button" data-copiar="00020126330014br.gov.bcb.pix0111390690838095204000053039865802BR5925EMERSON DE CARVALHO ASSIS6015SAO JOSE DO RIO62140510CAMFACILIA6304FBB7" data-copiado="Código copiado">Pix copia e cola</button>
        </div>
        <p class="pix__nota">Você escolhe o valor no app do banco. Fora do Brasil? Use o <a href="https://buymeacoffee.com/emersoncarvalho" target="_blank" rel="noopener">Buy Me a Coffee</a>.</p>
      </div>
    </div>

=== "O que o seu apoio libera"

    | Apoio | O que você recebe |
    |---|---|
    | **Doação livre** · qualquer valor | Seu nome e o seu recado no mural, se você quiser |
    | **Até R$ 500** · pedido de funcionalidade | Sua sugestão de melhoria é lida e avaliada por mim |
    | **A partir de R$ 1.000** · conversa | Uma hora de conversa comigo pelo Google Meet sobre o seu projeto |
    | **A partir de R$ 5.000** · desenvolvimento | Desenvolvemos juntos uma aplicação para você, com base no CamFácil IA |

    Os **10 primeiros** apoiadores viram **Apoiadores Fundadores**, com selo para sempre no mural.

    !!! tip "Ideias e pedidos de funcionalidade passam por aqui"
        Quer uma função nova? Faça o pedido junto com o apoio, na aba **Entrar no mural**. Prefere fazer você mesmo? O código é aberto: desenvolva e abra um [pull request](https://github.com/emersoncassis/CamFacil-IA/pulls), que eu avalio quando possível. Pedidos seguem o escopo do projeto e não têm prazo garantido.

=== "Entrar no mural"

    Fez o Pix? Preencha, anexe o comprovante e envie. Chega direto para mim.

    <form class="apoio-form" data-msg-enviando="Enviando…" data-msg-ok="Recebido! Muito obrigado pelo apoio. Seu nome entra no mural depois que eu conferir." data-msg-erro="Não foi possível enviar agora. Tente de novo em instantes." data-msg-grande="O comprovante passou de 6 MB. Mande um print menor." data-titulo="Apoio ao CamFácil IA" data-assunto="Apoio ao CamFácil IA" data-anexo="(comprovante em anexo)">
      <div class="apoio-grade">
        <label>Nome para o mural<input data-campo="Nome" name="nome" required maxlength="60"></label>
        <label>Instagram ou LinkedIn (opcional)<input data-campo="Link" name="link" maxlength="120"></label>
        <label>Valor<input data-campo="Valor" name="valor" inputmode="decimal" placeholder="R$" maxlength="20"></label>
        <label>Tipo de apoio
          <select data-campo="Tipo" name="tipo">
            <option>Doação livre</option>
            <option>Pedido de funcionalidade</option>
            <option>Conversa de uma hora</option>
            <option>Desenvolvimento juntos</option>
          </select>
        </label>
        <label class="largo">Recado<textarea data-campo="Recado" name="recado" rows="1" maxlength="400"></textarea></label>
        <label class="largo">O que você quer? Uma funcionalidade, o assunto da conversa ou a aplicação (opcional)<textarea data-campo="O que quer" name="pedido" rows="1" maxlength="600"></textarea></label>
        <label class="largo">Comprovante (foto ou PDF)<input type="file" name="comprovante" accept="image/*,application/pdf"></label>
        <input class="apoio-isca" name="site" tabindex="-1" autocomplete="off" aria-hidden="true">
      </div>
      <label class="apoio-ok"><input type="checkbox" name="publicar" data-campo="Publicar no mural" data-sim="sim, autorizo" data-nao="não" checked> Autorizo publicar meu nome, link e recado no mural</label>
      <div class="apoio-botoes">
        <button class="md-button md-button--primary" type="submit" data-enviar>Enviar</button>
        <span class="apoio-aviso" role="status" aria-live="polite"></span>
      </div>
    </form>

=== "Mural"

    Obrigado a quem acredita que a IA de vídeo também é do técnico.

    --8<-- "mural.md"
