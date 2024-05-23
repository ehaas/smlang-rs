extern crate smlang;

use derive_more::Display;

use smlang::statemachine;

#[test]
fn compile_fail_tests() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile-fail/*.rs");
}

#[test]
fn wildcard_after_input_state() {
    statemachine! {
        transitions: {
            *State1 + Event1 = State2,
            _ + Event1 = Fault,
        }
    }

    struct Context;
    impl StateMachineContext for Context {}

    let mut sm = StateMachine::new(Context);

    sm.process_event(Events::Event1).unwrap();
    assert!(matches!(sm.state(), Ok(&States::State2)));

    sm.process_event(Events::Event1).unwrap();
    assert!(matches!(sm.state(), Ok(&States::Fault)));
}

#[test]
fn multiple_lifetimes() {
    pub struct X;
    pub struct Y;
    pub struct Z;

    statemachine! {
        transitions: {
            *State1 + Event1(&'a X) [guard1] / action1 = State2(&'a X),
            State2(&'a X) + Event2(&'b Y) [guard2] / action2 = State3((&'a X, &'b Y)),
            State4 + Event(&'c Z) [guard3] / action3 = State5,
        }
    }

    #[allow(dead_code)]
    struct Context;

    impl StateMachineContext for Context {
        fn guard1(&mut self, _event_data: &X) -> Result<bool, ()> {
            Ok(true)
        }

        fn guard2(&mut self, _state_data: &X, _event_data: &Y) -> Result<bool, ()> {
            Ok(true)
        }

        fn guard3(&mut self, _event_data: &Z) -> Result<bool, ()> {
            Ok(true)
        }

        fn action1<'a>(&mut self, event_data: &'a X) -> &'a X {
            event_data
        }

        fn action2<'a, 'b>(&mut self, state_data: &'a X, event_data: &'b Y) -> (&'a X, &'b Y) {
            (state_data, event_data)
        }

        fn action3(&mut self, _event_data: &Z) {}
    }

    #[allow(dead_code)]
    struct WrappedStates<'a, 'b>(States<'a, 'b>);

    #[allow(dead_code)]
    struct WrappedEvents<'a, 'b, 'c>(Events<'a, 'b, 'c>);
}

#[test]
fn derive_display_events_states() {
    statemachine! {
        derive_events: [Debug,Display],
        derive_states: [Debug,Display],
        transitions: {
            *Init + Event = End,
        }
    }

    struct Context;
    impl StateMachineContext for Context {}

    let mut sm = StateMachine::new(Context);
    assert_eq!(format!("{}", sm.state().unwrap()), "Init");
    assert_eq!(format!("{:?}", sm.state().unwrap()), "Init");

    let event = Events::Event;
    assert_eq!(format!("{}", event), "Event");

    sm.process_event(event).unwrap();
    assert!(matches!(sm.state(), Ok(&States::End)));
    assert_eq!(format!("{}", sm.state().unwrap()), "End");
}

#[test]
fn named_derive_display_events_states() {
    statemachine! {
        name: SM,
        derive_events: [Debug,Display],
        derive_states: [Debug,Display],
        transitions: {
            *Init + Event = End,
        }
    }

    struct Context;
    impl SMStateMachineContext for Context {}

    let mut sm = SMStateMachine::new(Context);
    assert_eq!(format!("{}", sm.state().unwrap()), "Init");
    assert_eq!(format!("{:?}", sm.state().unwrap()), "Init");

    let event = SMEvents::Event;
    assert_eq!(format!("{}", event), "Event");

    sm.process_event(event).unwrap();
    assert!(matches!(sm.state(), Ok(&SMStates::End)));
    assert_eq!(format!("{}", sm.state().unwrap()), "End");
}

#[test]
fn async_guards_and_actions() {
    use smol;

    smol::block_on(async {
        statemachine! {
            transitions: {
                *State1 + Event1 [async guard1] / async action1 = State2,
                _ + Event1 = Fault,
            }
        }

        struct Context;
        #[smlang::async_trait]
        impl StateMachineContext for Context {
            async fn guard1(&mut self) -> Result<bool, ()> {
                Ok(true)
            }

            async fn action1(&mut self) -> () {
                ()
            }
        }

        let mut sm = StateMachine::new(Context);

        sm.process_event(Events::Event1).await.unwrap();
        assert!(matches!(sm.state(), Ok(&States::State2)));

        sm.process_event(Events::Event1).await.unwrap();
        assert!(matches!(sm.state(), Ok(&States::Fault)));
    });
}
