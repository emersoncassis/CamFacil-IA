---
title: "CamFácil IA Builder: build your own version without coding"
description: "The CamFácil IA robot technician: you say what you want and it builds screens, features and detections in your version, within privacy law and the license. Made with Reversa."
image: "assets/img/builder-og-en.png"
image_alt: "CCTV technician robot with a dome camera on its head writing code on a laptop, next to the line: You say what you want. It builds."
---

# CamFácil IA Builder

<div class="builder" markdown>
<div class="builder__arte">
  <img src="../../assets/img/builder.svg" alt="CamFácil IA Builder: a CCTV technician robot, with a dome camera on its head and a screwdriver in its hand, writes code on a laptop." width="640" height="440">
</div>
<div class="builder__texto" markdown>
<span class="manifesto__kicker">The project's master builder</span>

<p class="builder__titulo">You say what you want. <em>It builds.</em></p>

The Builder is a skill for coding assistants (Claude Code, Codex and others). You talk to it the way you would talk to a fellow installer, and it changes screens and creates features and detections in **your version** of CamFácil IA. You never write a line of code. It works in Brazilian Portuguese by default; ask it to answer in your language.

[:fontawesome-brands-github: Fork the repository](https://github.com/emersoncassis/CamFacil-IA/fork){ .md-button .md-button--primary }
[See the skill](https://github.com/emersoncassis/CamFacil-IA/blob/main/.claude/skills/camfacil-ia-builder/SKILL.md){ .md-button }
</div>
</div>

## Start in three steps

1. **Fork** the repository on GitHub and download your copy:

    ```sh
    git clone https://github.com/YOUR-USER/CamFacil-IA && cd CamFacil-IA
    ```

2. **Open the folder in your coding assistant.** In Claude Code, just run `claude` inside it. Other assistants read the `AGENTS.md` file, which points to the Builder.

3. **Call the Builder** and say what you want:

    ```text
    /camfacil-ia-builder I want an alert when someone stands in front of the gate for more than 2 minutes
    ```

It checks the tools on your computer, installs what is missing, opens the original program so you can see the "before", and only then starts.

## What you can ask for

??? example "Change how the program looks"
    Replace the name, logo, colors and texts with your company's, rearrange screens, create a mode for the customer's TV. Colors come from a single file, so the change applies to the whole program.

??? example "Create a new detection"
    A new kind of alert about people and vehicles (line, zone, dwell time, counting), or a model you trained yourself in studio mode.

??? example "Connect to other systems"
    A new camera or recorder brand, an alert on another channel, a webhook for the customer's automation.

??? example "Reports and maintenance"
    A report with your brand, one more network diagnostic, a camera health screen the way you explain it to the customer.

## What it won't do, and why

The Builder refuses a request that breaks the law, the license or security. It names **which rule** and offers **what can be done instead**.

| You ask | It answers |
|---|---|
| Recognize who a person is by their face | No. Biometrics are sensitive data (LGPD, art. 11). Instead: detect presence, dwell time and line crossing, without identifying anyone. |
| Remove the warning sign or the terms | No. The LGPD requires transparency with the people being filmed. It customizes the text with your brand, but does not remove it. |
| Access a camera that is not the customer's | No. Breaking into a device is a crime (Brazilian Penal Code, art. 154-A). |
| Close the source of your version | No. The AGPL-3.0 license requires open source, including for people who use it over the network. |
| Remove the PIN for network access | No. A hole in your version is an open door at the customer's shop. |
| Multi-customer dashboard, continuous recording | Out of scope for the official project. In your own version it builds them, after telling you how your responsibilities change. |

The rules cite Brazilian law, where the project was born. If you work elsewhere, your local privacy law applies too. Full list (in Portuguese): [Builder limits](https://github.com/emersoncassis/CamFacil-IA/blob/main/.claude/skills/camfacil-ia-builder/referencias/limites.md). This is technical guidance, not legal advice.

## How it works

The Builder follows the **Reversa** method: the agreement first, the code after.

1. **Understands** the request and repeats it in its own words, for you to confirm.
2. **Checks** it against the law, the license, security and the project rules.
3. **Specifies** the feature and shows the plan in a few lines. It only goes on with your go-ahead.
4. **Builds** in small steps, on a test copy. The original is never lost.
5. **Proves** it: runs the tests, opens the program and shows it working.
6. **Documents** what changed, so the next person (or the next AI) can carry on.

## Your version, your obligations

- **It stays free.** Your version remains AGPL-3.0, with open source and the credits preserved.
- **Say it is a modified version**, with your name and the date. The brand the customer sees is yours, never a manufacturer's.
- **Privacy compliance is on you.** Use the [term, authorization and sign templates](privacidade.md).
- **Improved something that helps everyone?** Send it back: see [Contributing](contribuindo.md).

<footer class="builder__creditos" markdown>
**Credits.** CamFácil IA Builder is built on [Reversa](https://github.com/sandeco/reversa), the specification reverse-engineering framework created by professor **Sandeco Macedo** ([@sandeco](https://github.com/sandeco)), under the MIT license. The Builder's whole development method comes from it. [Reversa documentation](https://sandeco.github.io/reversa/) · [Paper](https://arxiv.org/abs/2605.18684)
</footer>
