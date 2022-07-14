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


<pre><font color="#4E9A06">goxlr-client</font> 0.1.0
Nathan Adams &lt;dinnerbone@dinnerbone.com&gt;, Craig McLure &lt;craig@mclure.net&gt;, Lars Mühlbauer
&lt;lm41@dismail.de&gt;
Allows control of a TC-Helicon GoXLR or GoXLR Mini, by interacting with a running daemon.

<font color="#C4A000">USAGE:</font>
    goxlr-client [OPTIONS] [SUBCOMMAND]

<font color="#C4A000">OPTIONS:</font>
        <font color="#4E9A06">--device</font> <font color="#4E9A06">&lt;DEVICE&gt;</font>    The specific device&apos;s serial number to execute commands on. This field
                             is optional if you have exactly one GoXLR, but required if you have
                             more
        <font color="#4E9A06">--status</font>             Display the device information after any subcommands have been executed
        <font color="#4E9A06">--status-json</font>        Display device information as JSON after command..
    <font color="#4E9A06">-h</font>, <font color="#4E9A06">--help</font>               Print help information
    <font color="#4E9A06">-V</font>, <font color="#4E9A06">--version</font>            Print version information

<font color="#C4A000">Microphone controls:</font>
        <font color="#4E9A06">--dynamic-gain</font> <font color="#4E9A06">&lt;DYNAMIC_GAIN&gt;</font>
            Set the gain of the plugged in dynamic (XLR) microphone. Value is in decibels and
            recommended to be lower than 72dB

        <font color="#4E9A06">--condenser-gain</font> <font color="#4E9A06">&lt;CONDENSER_GAIN&gt;</font>
            Set the gain of the plugged in condenser (XLR with phantom power) microphone. Value is
            in decibels and recommended to be lower than 72dB

        <font color="#4E9A06">--jack-gain</font> <font color="#4E9A06">&lt;JACK_GAIN&gt;</font>
            Set the gain of the plugged in jack (3.5mm) microphone. Value is in decibels and
            recommended to be lower than 72dB

<font color="#C4A000">SUBCOMMANDS:</font>
    <font color="#4E9A06">profiles</font>        Profile Settings
    <font color="#4E9A06">microphone</font>      Adjust the microphone settings (Eq, Gate and Compressor)
    <font color="#4E9A06">volume</font>          Adjust Channel Volumes
    <font color="#4E9A06">bleep-volume</font>    Configure the Bleep Button
    <font color="#4E9A06">faders</font>          Commands to manipulate the individual GoXLR Faders
    <font color="#4E9A06">cough-button</font>    Commands for configuring the cough button
    <font color="#4E9A06">router</font>          Commands to manipulate the GoXLR Router
    <font color="#4E9A06">lighting</font>        Commands to control the GoXLR lighting
    <font color="#4E9A06">help</font>            Print this message or the help of the given subcommand(s)</pre>

