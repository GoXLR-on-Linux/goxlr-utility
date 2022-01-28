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
    [0x80b] = "SetMicrophoneType",
    [0x80c] = "GetMicrophoneLevel",
    [0x80f] = "GetHardwareInfo",
    [0x814] = "SetFaderDisplayMode",
}

goxlr_protocol = Proto("GoXLR", "GoXLR USB protocol")

local f_header = ProtoField.bytes("goxlr.header", "Header")
local f_header_command = ProtoField.uint24("goxlr.header.command", "Command", base.HEX, COMMAND_NAMES)
local f_header_subcommand = ProtoField.uint24("goxlr.header.subcommand", "Subcommand", base.HEX)
local f_header_length = ProtoField.uint16("goxlr.header.length", "Body Length", base.DEC)
local f_command_index = ProtoField.uint16("goxlr.header.index", "Index", base.DEC)
local f_body = ProtoField.bytes("goxlr.body", "Body")
local f_body_effect = ProtoField.bytes("goxlr.body", "Effect")
local f_body_effect_key = ProtoField.uint16("goxlr.body.effect.key", "Effect Key", base.HEX)
local f_body_effect_value = ProtoField.uint16("goxlr.body.effect.value", "Effect Value", base.HEX)

local f_data_fragment = Field.new("usb.data_fragment")
local f_control_response = Field.new("usb.control.Response")

goxlr_protocol.fields = { f_header, f_header_command, f_header_subcommand, f_header_length, f_command_index, f_body, f_body_effect, f_body_effect_key, f_body_effect_value }

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
    local length = buffer:len()

    pinfo.cols.protocol = goxlr_protocol.name

    local subtree = tree:add(goxlr_protocol, buffer(), "GoXLR Command")
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
                local effect = body:add(f_body_effect, effect_buffer)
                effect:add_le(f_body_effect_key, effect_buffer(0, 4))
                effect:add_le(f_body_effect_value, effect_buffer(4, 4))
            end
        end
    end
end

register_postdissector(goxlr_protocol)

