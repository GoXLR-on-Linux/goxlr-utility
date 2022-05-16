# GoXLR Notes

This document is mostly a scribble pad, to document certain behaviours prior to them being implemented in the daemon,
it'll mostly be discussing how the profile -> device behaviours occur under Windows.

### Mute to X
General behaviour here seems to be pretty simple, it's essentially a dynamic, transient runtime update to the routing
table which removes the fader source to the X output, and then puts it back when unmuted. This change shouldn't be
stored in the config (outside the mute state attributes), or represented in any way as to imply it's a permanent change.

*Implementation Thoughts*: When applying routing table changes, have calls to methods which can adjust the actual sent
commands based on their state. Care needs to be taken to ensure that if a 'Mute to X' is configured for a route that was
never actually enabled / configured, nothing is done.


### Cough (Mic Mute) Button
Despite seemingly controlling the same way as a fader mute button does, the main difference is that it will not adjust
the volume of a channel, instead just set the muted state. In addition, if the microphone is attached to a fader, it will
not interfere with the fader's behaviour (a cough button mute is considered separate to a fader mute).

The `<muteChat>` tag of the profile defines general behaviour, but uses the `coughButtonIsOn` attribute rather than
`muteChatstate` to define whether it's in use or not, it still however uses the `blink` attribute for 'Mute to All'
behaviour.

Mute to X behaviour is available for both Hold and Toggle, with toggle providing additional 'mute to all' behaviour.

*Implementation Thoughts*: Consideration needs to be given over the 'ordering' of the mic mute if it's also assigned to
a fader. Both the fader and cough button need to be checked to determine if the channel should be muted (so ensure that
unmuting the fader doesn't unmute the mic if the cough button is in toggle and 'On' mode), same applies to the Mute To X
behaviour, especially if a possible Mute to Y is in play.

### General States
A lot of tags have the `state` attribute, including tags which arguably shouldn't need them (`logoX`, `anEncoder`, etc)
which originally lead me to assume they were specifically colour map related. If a state was set to 1, the button would
light up, if it was set to 0 it wouldn't.

On further research, it does appear that these serve a dual purpose. As well as for colours, in some cases it appears 
that the GoXLR app under Windows uses this state to handle additional behaviour, it works as the boolean to say whether
something is enabled or not.

*Implementation Thoughts*: While strictly speaking not accurate, leaving these in the Colour Map allows us to implement
colour handling correctly, as well as an easy way to check on a state, so no work needed here.

### Fader Mute States
These are interesting, as well as the `state` attribute mentioned above, they also use the `blink` attribute to
determine whether 'Mute to All' is active. 'Mute to All' activates when the mute button is held down for around a second.
We ultimately end up with 3 possible states for a faders mute button:

1) `state=0` - Unmuted
2) `state=1` - Mute to X
3) `state=1` && `blink=1` - Mute to All

In my testing, there should never be a situation where `state=0` and `blink=1`.

#### Volume Sliding / Channel Muting
From my testing, the volume slider on the GoXLR full will only ever 'descend' in a 'Mute to All' state. This state can 
be provided as part of `MuteFunction::All` for `state=1 && blink=0`, or occur when `state=1 && blink=1`, however, outside the faders, muting a channel doesn't appear to be a direct GoXLR feature (either via the UI, or button presses) but is
something we support via the CLI, and the volume change seems to be purely an asthetic feature of the full sized GoXLR.

*Implementation Thoughts*: A check should be performed, in the case of the GoXLR Mini we should never change the volumes
as there's no motorised faders to adjust which simplifies the mute / unmute process, in addition if a channel mute occurs
that isn't attached to a fader, just mute the channel, don't adjust the volume (saves us having to maintain an internal
state for all channels). A microphone fader needs a couple of additional checks to keep the cough button and fader button
separate.

## Fader Assignment
When assigning faders a couple of steps occur, if a fader is muted in any way and is assigned off the faders, the channel
is unmuted and restored immediately after the switch.

In addition, the GoXLR software prevents the same fader being assigned to multiple channels, in the event a fader is 
changed with a fader that's already present, the two switch.
