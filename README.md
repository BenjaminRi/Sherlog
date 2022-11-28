# Sherlog

## Introduction

Sherlog visualizes log files. The point of this program is to provide a rich GUI for developers to analyze their systems. Log files often come in text form and various formats. Given the right set of parsers, Sherlog provides a way to look at and intersperse these log files while providing filters and sort functions. This allows a better insight into systems that generate logs.

![Sherlog GUI screenshot](/doc/sherlog_gui_screenshot.png)

Sherlog uses the notion of log sources. A log source represents either a group of child log sources or it contains log entries. A log source is like a folder in a file system and the GUI visualizes them in the familiar tree structure known from file explorers. A log entry mainly consists of a timestamp, a severity and a text message.

## State of the implementation

The parsers are done, the GUI is functional, albeit minimal, and has been in daily use by various people for over a year. The implementation of GUI improvements and new features is ongoing. Sherlog is still considered beta software, but is not far from a 1.0 release. The program is written in Rust, backed by GTK+ 3 to display the GUI. It is compatible with Windows, Linux and all other operating systems that support Rust and GTK+ 3.

## How to compile

### Windows

If you haven't already installed Rust, it is recommended to do so via `rustup-init.exe`, which can be downloaded on the [official Rust website](https://www.rust-lang.org/tools/install). Select the GNU toolchain `x86_64-pc-windows-gnu` (where `x86_64` is the architecture of your computer, adjust if necessary) by entering it as the *default host triple*. You can either specify this during the initial installation, or change it later via `rustup` by installing the toolchain with

```sh
rustup toolchain install stable-x86_64-pc-windows-gnu
```

and then selecting it with

```sh
rustup default stable-x86_64-pc-windows-gnu
```

Install [MSYS2](https://www.msys2.org/). For simplicity, this tutorial will assume that you installed it in the directory `C:\msys64`. Start the MSYS2 console `C:\msys64\msys2.exe` and run the following commands inside that console, confirming the install prompts:

```sh
pacman -Syu # Update all packages, if necessary
pacman -S mingw-w64-x86_64-gcc mingw-w64-x86_64-pkgconf mingw-w64-x86_64-gtk3
```

After that, you can close the MSYS2 console. Depending on your needs, you can choose to work in the Windows console or, if you prefer the GNU/Linux environment instead, use the MSYS2 Mingw console.

#### Windows console

Open a Windows console (`cmd.exe`).

Make the newly installed binaries available in the path variable.

```sh
SET PATH=%PATH%;C:\msys64\mingw64\bin
```

Use `SETX` to persist this change over console and computer restarts.
```sh
SETX PATH %PATH%
```

Build the project by `cd`ing to the project folder and running `cargo build`.

#### MSYS2 Mingw console

Launch the `C:\msys64\mingw64.exe` console and make cargo available in the path variable with

```sh
PATH="${PATH}:/c/Users/${USER}/.cargo/bin"
```

You may want to append this command to your `.bashrc` to persist this change over console and computer restarts.

```sh
echo 'PATH="${PATH}:/c/Users/${USER}/.cargo/bin"' >> "/home/${USER}/.bashrc"
```

Build the project by `cd`ing to the project folder and running `cargo build`.

### Linux

On Linux, install the [Rust compiler](https://www.rust-lang.org/tools/install).

Then you need to get a linker (this will come with build-essential) and the GTK development libraries and header files. On Debian/Ubuntu, this is done via APT:

```sh
sudo apt install build-essential libgtk-3-dev
```

Build the project by `cd`ing to the project folder and running `cargo build`.

### Mac

On Mac, install the [Rust compiler](https://www.rust-lang.org/tools/install).

Get the GTK development libraries:

```sh
brew install gtk+3
```

Build the project by `cd`ing to the project folder and running `cargo build`.

### Cross-compilation

Cross-compilation, like building on a Linux host for Windows targets, is possible and works. Setting this up is left as an exercise for the reader.

### Troubleshooting

If something fails to build, it is most likely gtk-rs, because the other dependencies are plain Rust code handled by cargo and do not depend on C libraries. The official [gtk-rs website](https://gtk-rs.org/) may be helpful in that case. Don't hesitate to connect to the IRC chat, where many talented and knowledgeable developers are active.
