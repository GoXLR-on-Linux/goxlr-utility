#!/bin/bash

# These checks will likely need expanding over time due to differing available configurations
function pulse_get_output() {
  # This is slightly 'legacy' UCM
  DEVICE=$(pactl list short sinks | grep goxlr_sample | awk '{print $2}');
  if [ -n "$DEVICE" ]; then
    echo "$DEVICE";
    exit 0;
  fi

  # This is 'current' UCM
  DEVICE=$(pactl list short sinks | grep GoXLR_0_8_9 | awk '{print $2}');
  if [ -n "$DEVICE" ]; then
    echo "$DEVICE";
    exit 0;
  fi

  # Couldn't find the device.
  >&2 echo "Unable to Find GoXLR Sample output Device";
  exit 1;
}

function pulse_get_input() {
  # Similar to above, look specifically for the sampler source (not currently present with Jack script)
  DEVICE=$(pactl list short sources | grep source | grep 'goxlr_sampler.*source' | awk '{print $2}');
  if [ -n "$DEVICE" ]; then
    echo "$DEVICE";
    exit 0;
  fi

  # This is 'current' UCM
  DEVICE=$(pactl list short sources | grep source | grep 'GoXLR_0_4_5.*source' | awk '{print $2}');
  if [ -n "$DEVICE" ]; then
    echo "$DEVICE";
    exit 0;
  fi


  >&2 echo "Unable to locate GoXLR Sampler input Device";
  exit 1;
}

function pulse_play_audio() {
  # Playback the specified file through paplay..
  paplay --volume=65536 -d "$DEVICE" "$FILE"
  exit 0;
}

function pulse_record_audio() {
  # Record to the specified file through parecord, keeping latency as low as possible..
  parecord --latency-msec=1 --volume=65535 -d "$DEVICE" "$FILE"
  exit 0;
}

function pipewire_get_output() {
  # pw-cli dump short Node

  >&2 echo "Pipewire Get Output Not Implemented";
  exit 1;
}

function pipewire_get_input() {
  # pw-cli dump short Node

  >&2 echo "Pipewire Get Input Not Implemented";
  exit 1;
}

case $1 in
  get-output-device)
    if [ -x "$(command -v pactl)" ]; then
      pulse_get_output
    elif [ -x "$(command -v pw-cli)" ]; then
      pipewire_get_output
    fi

    >&2 echo "Unable to locate a compatible command to find output device";
    exit 1;

    ;;
  get-input-device)
    if [ -x "$(command -v pactl)" ]; then
      pulse_get_input
    elif [ -x "$(command -v pw-cli)" ]; then
      pipewire_get_input
    fi

    >&2 echo "Unable to locate a compatible command to find input device";
    exit 1;

    ;;
  play-file)
    DEVICE=$2
    FILE=$3

    if [ -x "$(command -v paplay)" ]; then
      pulse_play_audio
    fi

    >&2 echo "Unable to locate a compatible command to play audio";
    exit 1;

    ;;
  record-file)
    DEVICE=$2
    FILE=$3

    if [ -x "$(command -v parecord)" ]; then
      pulse_record_audio
    fi

    >&2 echo "Unable to locate a compatible command to record audio";
    exit 1;
    ;;
esac


>&2 echo "Attempted to perform an unsupported action";
exit 1
