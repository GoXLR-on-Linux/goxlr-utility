// This file primarily wraps USB information into some 'Plain Old Rust Structs' which can be used
// by other modules to interrogate devices. The idea here is that they shouldn't be aware of what
// goes on 'Under the Hood' of the GoXLR device, nor the communication layer (they shouldn't need
// to poll USB directly).
