pub mod data;
pub mod event;
pub mod input_state;
pub mod lifetimes;
pub mod output_state;
pub mod state_machine;
pub mod transition;

use data::DataDefinitions;
use event::EventMapping;
use state_machine::StateMachine;

use input_state::InputState;
use proc_macro2::{Span, TokenStream};

use crate::parser::event::Transition;
use std::collections::{hash_map, HashMap};
use std::fmt;
use syn::{parse, Attribute, Ident, Type};
use transition::StateTransition;
pub type TransitionMap = HashMap<String, HashMap<String, EventMapping>>;

#[derive(Debug, Clone)]
pub struct AsyncIdent {
    pub ident: Ident,
    pub is_async: bool,
}
impl AsyncIdent {
    pub fn to_token_stream<F>(&self, visit: &mut F) -> TokenStream
    where
        F: FnMut(&AsyncIdent) -> TokenStream,
    {
        visit(self)
    }
}
impl fmt::Display for AsyncIdent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_async {
            write!(f, "{}().await", self.ident)
        } else {
            write!(f, "{}()", self.ident)
        }
    }
}

#[derive(Debug)]
pub struct ParsedStateMachine {
    pub name: Option<Ident>,
    pub states_attr: Vec<Attribute>,
    pub events_attr: Vec<Attribute>,
    pub temporary_context_type: Option<Type>,
    pub custom_error: bool,
    pub states: HashMap<String, Ident>,
    pub starting_state: Ident,
    pub state_data: DataDefinitions,
    pub events: HashMap<String, Ident>,
    pub event_data: DataDefinitions,
    pub states_events_mapping: HashMap<String, HashMap<String, EventMapping>>,
    pub entry_exit_async: bool,
}

// helper function for adding a transition to a transition event map
fn add_transition(
    transition: &StateTransition,
    transition_map: &mut TransitionMap,
    state_data: &DataDefinitions,
) -> Result<(), parse::Error> {
    let p = transition_map
        .get_mut(&transition.in_state.ident.to_string())
        .unwrap();

    match p.entry(transition.event.ident.to_string()) {
        hash_map::Entry::Vacant(entry) => {
            let mapping = EventMapping {
                in_state: transition.in_state.ident.clone(),
                event: transition.event.ident.clone(),
                transitions: vec![Transition {
                    guard: transition.guard.clone(),
                    action: transition.action.clone(),
                    out_state: transition.out_state.ident.clone(),
                }],
            };
            entry.insert(mapping);
        }
        hash_map::Entry::Occupied(mut entry) => {
            let mapping = entry.get_mut();
            mapping.transitions.push(Transition {
                guard: transition.guard.clone(),
                action: transition.action.clone(),
                out_state: transition.out_state.ident.clone(),
            });
        }
    }

    // Check for actions when states have data a
    if state_data
        .data_types
        .contains_key(&transition.out_state.ident.to_string())
    {
        // This transition goes to a state that has data associated, check so it has an
        // action

        if transition.action.is_none() {
            return Err(parse::Error::new(
                transition.out_state.ident.span(),
                "This state has data associated, but not action is define here to provide it.",
            ));
        }
    }
    Ok(())
}

impl ParsedStateMachine {
    pub fn new(mut sm: StateMachine) -> parse::Result<Self> {
        // Derive out_state for internal non-wildcard transitions
        for transition in sm.transitions.iter_mut() {
            if transition.out_state.internal_transition && !transition.in_state.wildcard {
                transition.out_state.ident = transition.in_state.ident.clone();
                transition
                    .out_state
                    .data_type
                    .clone_from(&transition.in_state.data_type);
                transition.out_state.internal_transition = false;
            }
        }

        // Check the initial state definition
        let mut starting_transitions_iter = sm.transitions.iter().filter(|sm| sm.in_state.start);

        let starting_transition = starting_transitions_iter.next().ok_or(parse::Error::new(
            Span::call_site(),
            "No starting state defined, indicate the starting state with a *.",
        ))?;

        if starting_transitions_iter.next().is_some() {
            return Err(parse::Error::new(
                Span::call_site(),
                "More than one starting state defined (indicated with *), remove duplicates.",
            ));
        }

        // Extract the starting state
        let starting_state = starting_transition.in_state.ident.clone();

        let mut states = HashMap::new();
        let mut state_data = DataDefinitions::new();
        let mut events = HashMap::new();
        let mut event_data = DataDefinitions::new();
        let mut states_events_mapping = TransitionMap::new();

        for transition in sm.transitions.iter() {
            // Collect states
            let in_state_name = transition.in_state.ident.to_string();
            if !transition.in_state.wildcard {
                states.insert(in_state_name.clone(), transition.in_state.ident.clone());
                state_data.collect(in_state_name.clone(), transition.in_state.data_type.clone())?;
            }
            if !transition.out_state.internal_transition {
                let out_state_name = transition.out_state.ident.to_string();
                states.insert(out_state_name.clone(), transition.out_state.ident.clone());
                state_data.collect(
                    out_state_name.clone(),
                    transition.out_state.data_type.clone(),
                )?;
            }

            // Collect events
            let event_name = transition.event.ident.to_string();
            events.insert(event_name.clone(), transition.event.ident.clone());
            event_data.collect(event_name.clone(), transition.event.data_type.clone())?;

            // add input and output states to the mapping HashMap
            if !transition.in_state.wildcard {
                states_events_mapping.insert(transition.in_state.ident.to_string(), HashMap::new());
            }
            if !transition.out_state.internal_transition {
                states_events_mapping
                    .insert(transition.out_state.ident.to_string(), HashMap::new());
            }
        }

        for transition in sm.transitions.iter() {
            // if input state is a wildcard, we need to add this transition for all states
            if transition.in_state.wildcard {
                let mut transition_added = false;

                for (name, in_state) in &states {
                    // skip already set input state
                    let p = states_events_mapping
                        .get_mut(&in_state.to_string())
                        .unwrap();

                    if p.contains_key(&transition.event.ident.to_string()) {
                        continue;
                    }

                    // create a new input state from wildcard
                    let in_state = InputState {
                        start: false,
                        wildcard: false,
                        ident: in_state.clone(),
                        data_type: state_data.data_types.get(name).cloned(),
                    };

                    // create the transition
                    let mut out_state = transition.out_state.clone();
                    if out_state.internal_transition {
                        out_state.ident = in_state.ident.clone();
                        out_state.data_type.clone_from(&in_state.data_type);
                    }
                    let wildcard_transition = StateTransition {
                        in_state,
                        event: transition.event.clone(),
                        guard: transition.guard.clone(),
                        action: transition.action.clone(),
                        out_state,
                    };

                    // add the wildcard transition to the transition map
                    // TODO:  Need to work on the span of this error, as it is being caused by the wildcard
                    // but won't show up at that line
                    add_transition(
                        &wildcard_transition,
                        &mut states_events_mapping,
                        &state_data,
                    )?;

                    transition_added = true;
                }

                // No transitions were added by expanding the wildcard,
                // so emit an error to the user
                if !transition_added {
                    return Err(parse::Error::new(
                        transition.in_state.ident.span(),
                        "Wildcard has no effect",
                    ));
                }
            } else {
                add_transition(transition, &mut states_events_mapping, &state_data)?;
            }
        }

        Ok(ParsedStateMachine {
            name: sm.name,
            states_attr: sm.states_attr,
            events_attr: sm.events_attr,
            temporary_context_type: sm.temporary_context_type,
            custom_error: sm.custom_error,
            states,
            starting_state,
            state_data,
            events,
            event_data,
            states_events_mapping,
            entry_exit_async: sm.entry_exit_async,
        })
    }
}
