local COMMAND_NAMES = {
    [0x000] = "SystemInfo",
    [0x800] = "GetButtonStates",
    [0x801] = "SetEffectParameters",
    [0x802] = "SetScribble",
    [0x803] = "SetColourMap",
    [0x804] = "SetRouting",
    [0x805] = "SetFader",
    [0x806] = "SetChannelVolume",
    [0x808] = "SetButtonStates",
    [0x809] = "SetChannelState",
    [0x80a] = "SetEncoderValue",
    [0x80b] = "SetMicrophoneParameters",
    [0x80c] = "GetMicrophoneLevel",
    [0x80f] = "GetHardwareInfo",
    [0x814] = "SetFaderDisplayMode",
}

local EFFECT_KEYS = {
    [0x0073] = "BleepLevel",
    [0x0010] = "MicGateMode", -- From what I can tell, no UI option for this.
    [0x0011] = "GateThreshold",
    [0x0014] = "GateEnabled", -- Used during 'Mic Setup' to disable the gate
    [0x0015] = "GateAttenuation",
    [0x0016] = "GateAttack",
    [0x0017] = "GateRelease",
    [0x0126] = "Equalizer31HzFrequency",
    [0x0127] = "Equalizer31HzValue",
    [0x00f8] = "Equalizer63HzFrequency",
    [0x00f9] = "Equalizer63HzValue",
    [0x0113] = "Equalizer125HzFrequency",
    [0x0114] = "Equalizer125HzValue",
    [0x0129] = "Equalizer250HzFrequency",
    [0x012a] = "Equalizer250HzValue",
    [0x0116] = "Equalizer500HzFrequency",
    [0x0117] = "Equalizer500HzValue",
    [0x011d] = "Equalizer1KHzFrequency",
    [0x011e] = "Equalizer1KHzValue",
    [0x012c] = "Equalizer2KHzFrequency",
    [0x012d] = "Equalizer2KHzValue",
    [0x0120] = "Equalizer4KHzFrequency",
    [0x0121] = "Equalizer4KHzValue",
    [0x0109] = "Equalizer8KHzFrequency",
    [0x010a] = "Equalizer8KHzValue",
    [0x012f] = "Equalizer16KHzFrequency",
    [0x0130] = "Equalizer16KHzValue",
    [0x013d] = "CompressorThreshold",
    [0x013c] = "CompressorRatio",
    [0x013e] = "CompressorAttack",
    [0x013f] = "CompressorRelease",
    [0x0140] = "CompressorMakeUpGain",
    [0x000b] = "DeEsser",
    [0x0076] = "ReverbAmount",
    [0x002f] = "ReverbDecay",
    [0x0037] = "ReverbEarlyLevel",
    [0x0039] = "ReverbTailLevel", -- Broken in official app? This is a guess.
    [0x0030] = "ReverbPredelay",
    [0x0032] = "ReverbLoColor",
    [0x0033] = "ReverbHiColor",
    [0x0034] = "ReverbHiFactor",
    [0x0031] = "ReverbDiffuse",
    [0x0035] = "ReverbModSpeed",
    [0x0036] = "ReverbModDepth",
    [0x002e] = "ReverbStyle",
    [0x0075] = "EchoAmount",
    [0x0028] = "EchoFeedback",
    [0x001f] = "EchoTempo",
    [0x0022] = "EchoDelayL",
    [0x0023] = "EchoDelayR",
    [0x0024] = "EchoFeedbackL",
    [0x0026] = "EchoXFBLtoR",
    [0x0025] = "EchoFeedbackR",
    [0x0027] = "EchoXFBRtoL",
    [0x001e] = "EchoSource",
    [0x0020] = "EchoDivL",
    [0x0021] = "EchoDivR",
    [0x002a] = "EchoFilterStyle",
    [0x005d] = "PitchAmount",
    [0x0167] = "PitchCharacter",
    [0x0159] = "PitchStyle",
    [0x0060] = "GenderAmount",
    [0x003c] = "MegaphoneAmount",
    [0x0040] = "MegaphonePostGain",
    [0x003a] = "MegaphoneStyle",
    [0x003d] = "MegaphoneHP",
    [0x003e] = "MegaphoneLP",
    [0x003f] = "MegaphonePreGain",
    [0x0041] = "MegaphoneDistType",
    [0x0042] = "MegaphonePresenceGain",
    [0x0043] = "MegaphonePresenceFC",
    [0x0044] = "MegaphonePresenceBW",
    [0x0045] = "MegaphoneBeatboxEnable",
    [0x0046] = "MegaphoneFilterControl",
    [0x0047] = "MegaphoneFilter",
    [0x0048] = "MegaphoneDrivePotGainCompMid",
    [0x0049] = "MegaphoneDrivePotGainCompMax",
    [0x0134] = "RobotLowGain",
    [0x0133] = "RobotLowFreq",
    [0x0135] = "RobotLowWidth",
    [0x013a] = "RobotMidGain",
    [0x0139] = "RobotMidFreq",
    [0x013b] = "RobotMidWidth",
    [0x0137] = "RobotHiGain",
    [0x0136] = "RobotHiFreq",
    [0x0138] = "RobotHiWidth",
    [0x0147] = "RobotWaveform",
    [0x0146] = "RobotPulseWidth",
    [0x0157] = "RobotThreshold",
    [0x014d] = "RobotDryMix",
    [0x0000] = "RobotStyle",
    [0x0059] = "HardtuneKeySource", -- Legacy Value? Always sent as 0, HardTune is configured via Routing now.
    [0x005a] = "HardTuneAmount",
    [0x005c] = "HardTuneRate",
    [0x005b] = "HardTuneWindow",
    [0x005e] = "HardTuneScale",
    [0x005f] = "HardTunePitchAmount",

    -- States
    [0x014e] = "RobotEnabled",
    [0x00d7] = "MegaphoneEnabled",
    [0x00d8] = "HardtuneEnabled",

    -- These are a pretty massive (but reasonable) assumption until tested, these values are only
    -- ever changed when the FX button is hit, but are consistently on if FX are on, and off if FX
    -- are off. Unlike the above, there's no other way to toggle them, either in the UI or other.
    [0x00d5] = "Encoder1Enabled",
    [0x00d6] = "Encoder2Enabled",
    [0x0150] = "Encoder3Enabled",
    [0x0151] = "Encoder4Enabled",

    -- These are unknown attributes, they only ever occur on profile load, not when changing settings..
    [0x0158] = "Unknown158", -- Executed with a value of 0, at the start prior to mic profile load
    [0x014b] = "Unknown14b", -- Executed with a value of 1 after mic profile load, prior to effects
}

local MIC_PARAM_KEYS = {
    [0x000] = "MicType",
    [0x001] = "DynamicGain",
    [0x002] = "CondenserGain",
    [0x003] = "JackGain",
    [0x30200] = "GateThreshold",
    [0x30400] = "GateAttack",
    [0x30600] = "GateRelease",
    [0x30900] = "GateAttenuation",
    [0x60200] = "CompressorThreshold",
    [0x60300] = "CompressorRatio",
    [0x60400] = "CompressorAttack",
    [0x60600] = "CompressorRelease",
    [0x60700] = "CompressorMakeUpGain",
    [0x70100] = "BleepLevel",
}

goxlr_protocol = Proto("GoXLR", "GoXLR USB protocol")

local f_header = ProtoField.bytes("goxlr.header", "Header")
local f_header_command = ProtoField.uint24("goxlr.header.command", "Command", base.HEX, COMMAND_NAMES)
local f_header_subcommand = ProtoField.uint24("goxlr.header.subcommand", "Subcommand", base.HEX)
local f_header_length = ProtoField.uint16("goxlr.header.length", "Body Length", base.DEC)
local f_command_index = ProtoField.uint16("goxlr.header.index", "Index", base.DEC)
local f_body = ProtoField.bytes("goxlr.body", "Body")
local f_body_effect = ProtoField.bytes("goxlr.body.effect", "Effect")
local f_body_effect_key = ProtoField.uint32("goxlr.body.effect.key", "Effect Key", base.HEX, EFFECT_KEYS)
local f_body_effect_value = ProtoField.int32("goxlr.body.effect.value", "Effect Value", base.DEC)
local f_body_mic_param = ProtoField.bytes("goxlr.bodymic_param", "Mic Param")
local f_body_mic_param_key = ProtoField.uint32("goxlr.body.mic_param.key", "Param Key", base.HEX, MIC_PARAM_KEYS)
local f_body_mic_param_value = ProtoField.float("goxlr.body.mic_param.value", "Param Value", base.DEC)
local f_request = ProtoField.framenum("goxlr.request", "Request Packet", base.NONE, frametype.REQUEST)
local f_response = ProtoField.framenum("goxlr.response", "Response Packet", base.NONE, frametype.RESPONSE)

local f_data_fragment = Field.new("usb.data_fragment")
local f_control_response = Field.new("usb.control.Response")

goxlr_protocol.fields = {
    f_header, f_header_command, f_header_subcommand, f_header_length, f_command_index,
    f_request, f_response,
    f_body,
    f_body_effect, f_body_effect_key, f_body_effect_value,
    f_body_mic_param, f_body_mic_param_key, f_body_mic_param_value,
}

local conversations = {}

function goxlr_protocol.dissector(buffer, pinfo, tree)
    data_fragment = f_data_fragment()
    control_response = f_control_response()
    if data_fragment then
        buffer = data_fragment.range
    elseif control_response then
        buffer = control_response.range
    else
        return 0
    end

    local addr_lo = pinfo.net_src
    local addr_hi = pinfo.net_dst

    if addr_lo > addr_hi then
        addr_hi,addr_lo = addr_lo,addr_hi
    end

    local command_index = buffer(6, 2):le_uint()
    local convo_id = tostring(addr_lo) .. " " .. tostring(addr_hi) .. " " .. command_index

    if not conversations[convo_id] then
        conversations[convo_id] = {}
    end
    --pinfo.conversation = conversations[convo_id] -- bug in wireshark. fun.

    local length = buffer:len()

    pinfo.cols.protocol = goxlr_protocol.name

    local subtree = tree:add(goxlr_protocol, buffer(), "GoXLR Command")

    if data_fragment then
        conversations[convo_id].request = pinfo.number
        if conversations[convo_id].response then
            subtree:add(f_response, conversations[convo_id].response)
        end
    else
        conversations[convo_id].response = pinfo.number
        if conversations[convo_id].request then
            subtree:add(f_request, conversations[convo_id].request)
        end
    end

    local header = subtree:add(f_header, buffer(0, 16))
    local command = buffer(0, 4):le_uint()
    local command_id = bit.band(bit.rshift(command, 12), 0xfff)
    local subcommand_id = bit.band(command, 0xfff)
    header:add_le(f_header_command, buffer(0, 4), command_id)
    header:add_le(f_header_subcommand, buffer(0, 4), subcommand_id)
    header:add_le(f_header_length, buffer(4, 2))
    header:add_le(f_command_index, buffer(6, 2))

    if length > 16 then
        local body = subtree:add(f_body, buffer(16))
        local body_buffer = buffer(16)

        if command_id == 0x801 then
            for i = 0, body_buffer:len() - 1, 8 do
                local effect_buffer = body_buffer(i, 8)
                local key = effect_buffer(0, 4)
                local value = effect_buffer(4, 4)
                local effect = body:add(f_body_effect, effect_buffer)
                effect:add_le(f_body_effect_key, key)
                effect:add_le(f_body_effect_value, value)
                local key_name = EFFECT_KEYS[key:le_uint()] or "Unknown"
                effect:append_text(" (Set " .. key_name .. " to " .. value:le_int() .. ")")
            end
        end

        if command_id == 0x80b then
            for i = 0, body_buffer:len() - 1, 8 do
                local param_buffer = body_buffer(i, 8)
                local key = param_buffer(0, 4)
                local decoded_key = key:le_int()
                local effect = body:add(f_body_mic_param, param_buffer)
                local value = param_buffer(4, 4)
                local decoded_value = value:le_float()
                if decoded_key == 0x001 or decoded_key == 0x002 or decoded_key == 0x003 then
                    value = param_buffer(6, 2)
                    decoded_value = value:le_int()
                end
                effect:add_le(f_body_mic_param_key, key)
                effect:add_le(f_body_mic_param_value, value, decoded_value)
                local key_name = MIC_PARAM_KEYS[key:le_uint()] or "Unknown"
                effect:append_text(" (Set " .. key_name .. " to " .. decoded_value .. ")")
            end
        end
    end
end

register_postdissector(goxlr_protocol)

