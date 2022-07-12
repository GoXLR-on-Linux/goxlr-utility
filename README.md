# GoXLR configuration utility
A tool to configure a GoXLR without requiring a Windows VM.

At this time, full device initialization is not possible on Linux. This utility still requires that you initialize on Windows, but allows you to configure it after the fact from within Linux.

# Warning
This utility is currently not 'user ready', it's extremely rough around the edges and has very little interactivity. You're welcome to experiment with this if you're comfortable working with Rust, but please do not request support at this time.

This project is also not supported by, or affiliated in any way with, TC-Helicon. For the official GoXLR software, please refer to their website. 

## Setting Permissions
Copy `50-goxlr.rules` to `/etc/udev/rules.d/` and then reload with `sudo udevadm control --reload-rules`.

You may need to unplug and replug the GoXLR afterwards, to allow the new permissions to take effect.

## Building from source
### Prerequisites
- Install [Rust](https://rustup.rs/)
- Install libusb
  - Debian/Ubuntu: `apt install libusb-dev`
  - Arch/Manjaro: `pacman -S libusb`
- Have a GoXLR :)

### Building
You can build with `cargo build`, or install the specific executables with:
- `cargo install --path daemon` for the daemon
- `cargo install --path client` for the client to interact with the daemon

Tab-complete files for your terminal of choice will be available after building.

## Running the daemon
You can start the daemon by executing `goxlr-daemon`.

Running the daemon as a service will be left as an exercise to the reader (we'll provide a systemd config later probably!).

If the daemon can't connect to your GoXLR device, check your device permissions (see above!).

## Interacting with the GoXLR
Once the daemon is running, you can run `goxlr-client` to configure the GoXLR at will from your terminal.

For an up-to-date list of command line arguments, try `goxlr-client --help`!

```
goxlr-client 0.1.0
Nathan Adams <dinnerbone@dinnerbone.com>, Craig McLure <craig@mclure.net>, Lars Mühlbauer
<lm41@dismail.de>
Allows control of a TC-Helicon GoXLR or GoXLR Mini, by interacting with a running daemon.

USAGE:
    goxlr-client [OPTIONS] [SUBCOMMAND]

OPTIONS:
        --device <DEVICE>    The specific device's serial number to execute commands on. This field
                             is optional if you have exactly one GoXLR, but required if you have
                             more
        --status             Display the device information after any subcommands have been executed
    -h, --help               Print help information
    -V, --version            Print version information

Profile Management:
        --list-profiles                     List all profiles available for loading
        --list-mic-profiles                 List all microphone profiles available for loading
        --load-profile <PROFILE>            Load a GoXLR Profile
        --load-mic-profile <MIC_PROFILE>    Load a GoXLR Microphone Profile
        --save-profile                      Saves the current configuration to disk
        --save-mic-profile                  Save the currently configured microphone profile to disk

Fader controls:
        --fader-a <FADER_A>    Assign fader A [possible values: mic, line-in, console, system, game,
                               chat, sample, music, headphones, mic-monitor, line-out]
        --fader-b <FADER_B>    Assign fader B [possible values: mic, line-in, console, system, game,
                               chat, sample, music, headphones, mic-monitor, line-out]
        --fader-c <FADER_C>    Assign fader C [possible values: mic, line-in, console, system, game,
                               chat, sample, music, headphones, mic-monitor, line-out]
        --fader-d <FADER_D>    Assign fader D [possible values: mic, line-in, console, system, game,
                               chat, sample, music, headphones, mic-monitor, line-out]

Channel volumes:
        --mic-volume <MIC_VOLUME>                    Set Mic volume (0-255)
        --line-in-volume <LINE_IN_VOLUME>            Set Line-In volume (0-255)
        --console-volume <CONSOLE_VOLUME>            Set Console volume (0-255)
        --system-volume <SYSTEM_VOLUME>              Set System volume (0-255)
        --game-volume <GAME_VOLUME>                  Set Game volume (0-255)
        --chat-volume <CHAT_VOLUME>                  Set Chat volume (0-255)
        --sample-volume <SAMPLE_VOLUME>              Set Sample volume (0-255)
        --music-volume <MUSIC_VOLUME>                Set Music volume (0-255)
        --headphones-volume <HEADPHONES_VOLUME>      Set Headphones volume (0-255)
        --mic-monitor-volume <MIC_MONITOR_VOLUME>    Set Mic-Monitor volume (0-255)
        --line-out-volume <LINE_OUT_VOLUME>          Set Line-Out volume (0-255)

Microphone controls:
        --dynamic-gain <DYNAMIC_GAIN>
            Set the gain of the plugged in dynamic (XLR) microphone. Value is in decibels and
            recommended to be lower than 72dB

        --condenser-gain <CONDENSER_GAIN>
            Set the gain of the plugged in condenser (XLR with phantom power) microphone. Value is
            in decibels and recommended to be lower than 72dB

        --jack-gain <JACK_GAIN>
            Set the gain of the plugged in jack (3.5mm) microphone. Value is in decibels and
            recommended to be lower than 72dB

SUBCOMMANDS:
    faders        Commands to manipulate the individual GoXLR Faders
    faders-all    Commands to manipulate all GoXLR faders at once
    router        Commands to manipulate the GoXLR Router
    volume        Adjust Channel Volumes
    help          Print this message or the help of the given subcommand(s)

```

## Interacting with the GoXLR on Web-GUI (beta)

When daemon is runing, go to :

[http://localhost:14564/](http://localhost:14564/)

![Beta interface](/docs/images/goxlr-web-ui.png)
