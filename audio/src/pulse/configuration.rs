use std::cell::RefCell;
use std::ops::Deref;
use std::rc::Rc;

use libpulse_binding as pulse;
use libpulse_binding::callbacks::ListResult;
use libpulse_binding::context::introspect::{SinkInfo, SourceInfo};
use libpulse_binding::context::{Context, FlagSet, State};
use libpulse_binding::mainloop::standard::{IterateResult, Mainloop};
use libpulse_binding::proplist::Proplist;

use crate::audio::AudioConfiguration;

pub struct PulseAudioConfiguration {
    main_loop: Rc<RefCell<Mainloop>>,
    context: Rc<RefCell<Context>>,
}

impl PulseAudioConfiguration {
    fn new() -> Self {
        // Connect to the PulseAudio Server..
        let app_name: &str = env!("CARGO_PKG_NAME");

        let mut proplist = Proplist::new().unwrap();
        proplist
            .set_str(pulse::proplist::properties::APPLICATION_NAME, &app_name)
            .unwrap();

        let main_loop = Rc::new(RefCell::new(
            Mainloop::new().expect("Failed to create MainLoop"),
        ));
        let context = Rc::new(RefCell::new(
            Context::new_with_proplist(main_loop.borrow().deref(), app_name, &proplist)
                .expect("Unable to create context"),
        ));

        context
            .borrow_mut()
            .connect(None, FlagSet::NOFLAGS, None)
            .expect("Failed to connect context");

        loop {
            match main_loop.borrow_mut().iterate(true) {
                IterateResult::Success(_) => {}
                IterateResult::Quit(_) | IterateResult::Err(_) => {
                    panic!("Failed to Connect to Pulse Audio!");
                }
            }

            match context.borrow().get_state() {
                State::Unconnected => {}
                State::Connecting => {}
                State::Authorizing => {}
                State::SettingName => {}
                State::Ready => {
                    break;
                }
                State::Failed => {}
                State::Terminated => {}
            }
        }

        // At this point, we're connected and ready to go :)
        Self { main_loop, context }
    }
}

impl AudioConfiguration for PulseAudioConfiguration {
    fn get_outputs(&mut self) -> Vec<String> {
        let found: Vec<String> = vec![];
        let wrapped = Rc::new(RefCell::new(found));
        let insider = wrapped.clone();

        let op = {
            self.context.borrow_mut().introspect().get_sink_info_list(
                move |sink_list: ListResult<&SinkInfo>| {
                    if let ListResult::Item(item) = sink_list {
                        if let Some(name) = &item.name {
                            insider.borrow_mut().push(name.parse().unwrap());
                        }
                    }
                },
            )
        };

        // Block here until the above closure has completed..
        while op.get_state() == pulse::operation::State::Running {
            self.main_loop.borrow_mut().iterate(true);
        }

        let unwrapped = wrapped.deref().borrow().clone();
        unwrapped
    }

    fn get_inputs(&mut self) -> Vec<String> {
        // Basically identical to the above, except getting the Sources..
        let found: Vec<String> = vec![];
        let wrapped = Rc::new(RefCell::new(found));
        let insider = wrapped.clone();

        let op = {
            self.context.borrow_mut().introspect().get_source_info_list(
                move |source_list: ListResult<&SourceInfo>| match source_list {
                    ListResult::Item(item) => {
                        if let Some(name) = &item.name {
                            insider.borrow_mut().push(name.parse().unwrap());
                        }
                    }
                    ListResult::End => {}
                    ListResult::Error => {}
                },
            )
        };

        // Block here until the above closure has completed..
        while op.get_state() == pulse::operation::State::Running {
            self.main_loop.borrow_mut().iterate(true);
        }

        let unwrapped = wrapped.deref().borrow().clone();
        unwrapped
    }
}

impl Drop for PulseAudioConfiguration {
    fn drop(&mut self) {
        // We need to disconnect our context before we go out of scope, otherwise we'll
        // segfault when libpulse tries to drop.
        self.context.borrow_mut().disconnect();
    }
}

pub fn get_configuration() -> PulseAudioConfiguration {
    PulseAudioConfiguration::new()
}
