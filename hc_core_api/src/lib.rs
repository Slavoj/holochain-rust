/*!
hc_core_api provides a library for container applications to instantiate and run holochain applications.

# Examples

``` rust
extern crate hc_core;
extern crate hc_core_api;
extern crate hc_dna;
extern crate hc_agent;

use hc_core_api::*;
use hc_dna::Dna;
use hc_agent::Agent;
use std::sync::{Arc, Mutex};
use hc_core::context::Context;
use hc_core::logger::SimpleLogger;
use hc_core::persister::SimplePersister;

// instantiate a new app

// need to get to something like this:
//let dna = hc_dna::from_package_file("mydna.hcpkg");

// but for now:
let dna = Dna::new();
let agent = Agent::from_string("bob");
let context = Context {
    agent: agent,
    logger: Arc::new(Mutex::new(SimpleLogger {})),
    persister: Arc::new(Mutex::new(SimplePersister::new())),
};
let mut hc = Holochain::new(dna,Arc::new(context)).unwrap();

// start up the app
hc.start().expect("couldn't start the app");

// call a function in the app
hc.call("some_fn");

// get the state
{
    let state = hc.state();

    // do some other stuff with the state here
    // ...
}

// stop the app
hc.stop().expect("couldn't stop the app");

```
*/

extern crate hc_agent;
extern crate hc_core;
extern crate hc_dna;

use hc_core::context::Context;
use hc_dna::Dna;
use std::sync::Arc;

/// contains a Holochain application instance
#[derive(Clone)]
pub struct Holochain {
    instance: hc_core::instance::Instance,
    context: Arc<hc_core::context::Context>,
    active: bool,
}

use hc_core::error::HolochainError;
use hc_core::nucleus::fncall;
use hc_core::nucleus::Action::*;
use hc_core::state::Action::*;
use hc_core::state::State;

impl Holochain {
    /// create a new Holochain instance
    pub fn new(dna: Dna, context: Arc<Context>) -> Result<Self, HolochainError> {
        let mut instance = hc_core::instance::Instance::new();
        let name = dna.name.clone();
        let action = Nucleus(InitApplication(dna));
        instance.dispatch(action);
        instance.consume_next_action()?;
        context.log(&format!("{} instantiated", name))?;
        let app = Holochain {
            instance,
            context,
            active: false,
        };
        Ok(app)
    }

    /// activate the Holochain instance
    pub fn start(&mut self) -> Result<(), HolochainError> {
        if self.active {
            return Err(HolochainError::InstanceActive);
        }
        self.active = true;
        Ok(())
    }

    /// deactivate the Holochain instance
    pub fn stop(&mut self) -> Result<(), HolochainError> {
        if !self.active {
            return Err(HolochainError::InstanceNotActive);
        }
        self.active = false;
        Ok(())
    }

    /// call a function in a zome
    pub fn call(&mut self, fn_name: &str) -> Result<(), HolochainError> {
        if !self.active {
            return Err(HolochainError::InstanceNotActive);
        }
        let call_data = fncall::Call::new(fn_name);
        let action = Nucleus(Call(call_data));
        self.instance.dispatch(action.clone());
        self.instance.consume_next_action()
    }

    /// checks to see if an instance is active
    pub fn active(&self) -> bool {
        self.active
    }

    /// return
    pub fn state(&mut self) -> Result<&State, HolochainError> {
        Ok(self.instance.state())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hc_agent::Agent as HCAgent;
    use hc_core::context::Context;
    use hc_core::logger::Logger;
    use hc_core::persister::SimplePersister;
    use std::fmt;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct TestLogger {
        log: Vec<String>,
    }

    impl Logger for TestLogger {
        fn log(&mut self, msg: String) {
            self.log.push(msg);
        }
    }

    // trying to get a way to print out what has been logged for tests without a read function.
    // this currently fails
    impl fmt::Debug for TestLogger {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "{:?}", self.log[0])
        }
    }

    fn test_context(agent: hc_agent::Agent) -> (Arc<Context>, Arc<Mutex<TestLogger>>) {
        let logger = Arc::new(Mutex::new(TestLogger { log: Vec::new() }));
        (
            Arc::new(Context {
                agent: agent,
                logger: logger.clone(),
                persister: Arc::new(Mutex::new(SimplePersister::new())),
            }),
            logger,
        )
    }

    #[test]
    fn can_instantiate() {
        let mut dna = Dna::new();
        dna.name = "TestApp".to_string();
        let agent = HCAgent::from_string("bob");
        let (context, test_logger) = test_context(agent.clone());
        let result = Holochain::new(dna.clone(), context.clone());
        let hc = result.clone().unwrap();
        assert!(!hc.active);
        assert_eq!(hc.context.agent, agent);
        let test_logger = test_logger.lock().unwrap();
        assert_eq!(format!("{:?}", *test_logger), "\"TestApp instantiated\"");

        match result {
            Ok(hc) => {
                assert_eq!(hc.instance.state().nucleus().dna(), Some(dna));
            }
            Err(_) => assert!(false),
        };
    }

    #[test]
    fn can_start_and_stop() {
        let dna = Dna::new();
        let agent = HCAgent::from_string("bob");
        let (context, _) = test_context(agent.clone());
        let mut hc = Holochain::new(dna.clone(), context).unwrap();
        assert!(!hc.clone().active());

        // stop when not active returns error
        let result = hc.stop();
        match result {
            Err(HolochainError::InstanceNotActive) => assert!(true),
            Ok(_) => assert!(false),
            Err(_) => assert!(false),
        }

        let result = hc.start();
        match result {
            Ok(_) => assert!(true),
            Err(_) => assert!(false),
        }
        assert!(hc.active());

        // start when active returns error
        let result = hc.start();
        match result {
            Err(HolochainError::InstanceActive) => assert!(true),
            Ok(_) => assert!(false),
            Err(_) => assert!(false),
        }

        let result = hc.stop();
        match result {
            Ok(_) => assert!(true),
            Err(_) => assert!(false),
        }
        assert!(!hc.active());
    }

    #[test]
    fn can_call() {
        let dna = Dna::new();
        let agent = HCAgent::from_string("bob");
        let (context, _) = test_context(agent.clone());
        let mut hc = Holochain::new(dna.clone(), context).unwrap();
        let result = hc.call("bogusfn");
        match result {
            Err(HolochainError::InstanceNotActive) => assert!(true),
            Ok(_) => assert!(false),
            Err(_) => assert!(false),
        }

        hc.start().expect("couldn't start");

        // always returns not implemented error for now!
        let result = hc.call("bogusfn");
        match result {
            Err(HolochainError::NotImplemented) => assert!(true),
            Ok(_) => assert!(true),
            Err(_) => assert!(false),
        };
    }

    #[test]
    fn can_get_state() {
        let dna = Dna::new();
        let agent = HCAgent::from_string("bob");
        let (context, _) = test_context(agent.clone());
        let mut hc = Holochain::new(dna.clone(), context).unwrap();

        let result = hc.state();
        match result {
            Ok(state) => {
                assert_eq!(state.nucleus().dna(), Some(dna));
            }
            Err(_) => assert!(false),
        };
    }
}
