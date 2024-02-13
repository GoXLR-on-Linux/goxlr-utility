#!/bin/bash

# This script should be run as sudo, as it configures the initialiser launch options..

# Install the launchctl services.
launchctl load -w com.goxlr-on-linux.goxlr-utility.initialiser.mini.plist
launchctl load -w com.goxlr-on-linux.goxlr-utility.initialiser.full.plist

# And for good measure, we'll perform an initial run of the initialiser..
'/Library/Application Support/com.github.goxlr-on-linux/goxlr-utility/goxlr-initialiser'