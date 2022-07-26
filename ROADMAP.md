# Roadmap
*Last Updated for 0.2.0 Release*

## Completed Features
* Initialisation of Devices under Linux<sup>1</sup>
* Mic and Main Profile Management (Load / Save / New)<sup>2</sup>
* Microphone Selection and Gain
* Fader Assignments
* Fader Mute buttons (Mute channel, Mute to X, Press and Hold)
* The 'Cough' Button (Hold / Toggle, Mute to X, Press and Hold)
* Bleep Button Volume
* Noise Gate and Compressor
* Microphone Equalizer
* Equalizer Fine Tune<sup>3</sup>
* Audio Routing
* Fader and Button colour configurations<sup>3</sup>

<sup>1</sup> Depending on how your GoXLR works, this may require a reboot.  
<sup>2</sup> Profiles are 'cross platform', so Windows profiles should work with the util, and vice versa  
<sup>3</sup> Currently only configurable via the `goxlr-client`


## Partial Completion
* Voice Effects
  * If loaded from a Windows profile, the Voice FX Buttons should work and configure appropriately. The effects banks,
    dials, buttons should all also work. Currently, it's not possible to configure or change the fine details of the 
    effects under Linux.
  
## Not Completed
* Sampler
  * Due to the nature of Linux audio, this is a little more complex to deal with. Some preliminary code has been created
    and functions correctly, however it's massively incomplete and complicated to use.
* Scribbles
  * Generally just a low priority, not overly complicated to achieve, but also not really that important, 
    will arrive soon!
* Lighting
  * Lighting Configuration is still missing for the 'Global' lights, Effects Dials and Scribbles.