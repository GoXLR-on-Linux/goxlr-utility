#!/bin/bash

# This script should be run as the user, and runs the Utility on Login

# Install the launchctl services.
launchctl load -w com.goxlr-on-linux.goxlr-utility.plist