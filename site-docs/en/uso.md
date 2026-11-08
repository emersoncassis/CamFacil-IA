# Getting started

The program on the customer's computer is for **operation** only. If you want to learn from scratch, start with the [AI training for CCTV](formacao.md), in the browser.

## Connect your first camera

1. **Open the program.** It starts on **Cameras** and searches the network by itself (ONVIF discovery).
2. **Pick the camera**, type user and password and click connect. The image shows up right away.
3. **Turn on AI whenever you like**: draw the line or the zone, or ask *"AI, suggest it for me"*.

No camera around? Use the **webcam**, a **video file** or the **demo scene**.

!!! tip "Saved login"
    Cameras sharing the same user and password connect in one go. The password is stored encrypted on the machine.

## What the customer sees

The customer never sees AI boxes or lines. They only see the **balloon with the rule's name**, which blinks on alert, with sound and a flashing screen.

## What's in the box

| Feature | What it does |
|---|---|
| **Live** | 1 to 4 camera mosaic with status below: device, AI, alerts and services |
| **Network diagnostics** | Finds cameras that changed IP by MAC, IP conflicts and default passwords |
| **Network diagram** | Draws the installation's single-line diagram and explains it by voice |
| **Telegram and WhatsApp** | Alerts and commands with a QR code (WhatsApp via Evolution API, on a dedicated SIM) |
| **TV mode** | Kiosk screen for the customer, with your logo |
| **Android app** | Becomes the camera server or watches a customer remotely through Tailscale |
| **Validation report** | PDF with your brand to close the sale |
| **MCP** | Configure everything by chatting with an AI (off by default) |
| **Studio mode** *(basic)* | Describe a new model, label, train on Colab and share it (study only) |

??? question "Does the program stay open all the time?"
    You can set it to start with the computer. The terminal window shows a live summary: CPU, memory and cameras.
