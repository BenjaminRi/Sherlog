# Sherlog

## Introduction

Sherlog visualizes log files. The point of this program is to provide a rich GUI for developers to analyze their systems. Log files often come in text form and various formats. Given the right set of parsers, Sherlog provides a way to look at and intersperse these log files while providing filters and sort functions. This allows a better insight into systems that generate logs.

Sherlog uses the notion of log sources. A log source represents either a group of child log sources or it contains log entries. A log source is like a folder in a file system and the GUI visualizes them in the familiar tree structure known from file explorers. A log entry mainly contains of a timestamp, a severity and a text message.

## State of the implementation

The program is written in Rust, backed by GTK+ 3 to display the GUI. So far, the log source viewer and the data structures are done. Visualizing, sorting and filtering the logs is a work in progress. The program is not in a usable state for end users (pre-alpha). It is compatible with Windows, Linux and all other operating systems that support Rust and GTK+ 3.

## How to compile

The Rust compiler is needed. Please follow the instructions at https://gtk-rs.org/ to set up an environment to link with GTK.