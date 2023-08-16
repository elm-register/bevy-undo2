use bevy::app::{App, Plugin, PostUpdate, PreUpdate};
use bevy::prelude::{Event, EventReader, EventWriter, in_state, IntoSystemConfigs, NextState, Res, ResMut, Resource, States};

use crate::counter::UndoCounter;
use crate::request::RequestUndoEvent;
use crate::reserve::{RequestCommitReservationsEvent, RequestCommitReservationsFromSchedulerEvent, ReserveCounter};
use crate::undo_event::UndoEvent;

mod counter;
mod extension;
mod request;
mod undo_event;
mod reserve;

pub mod prelude {
    pub use crate::extension::AppUndoEx;
    pub use crate::request::{UndoRequester, RequestUndoEvent};
    pub use crate::undo_event::{UndoReserveCommitter, UndoScheduler};
    #[cfg(feature = "callback_event")]
    pub use crate::undo_event::callback::UndoCallbackEvent;
    pub use crate::UndoPlugin;
}


/// Add undo-operations to an app.
#[derive(Debug, Default, Eq, PartialEq, Copy, Clone, Hash)]
pub struct UndoPlugin;


impl Plugin for UndoPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_state::<UndoState>()
            .add_event::<RequestUndoEvent>()
            .add_event::<RequestCommitReservationsFromSchedulerEvent>()
            .add_event::<RequestCommitReservationsEvent>()
            .add_event::<UndoWaitEvent>()
            .init_resource::<UndoCounter>()
            .init_resource::<Posted>()
            .add_systems(PreUpdate, (request_undo_system, undo_wait_event_system)
                .chain()
                .run_if(in_state(UndoState::None)),
            )
            .add_systems(PostUpdate, reset_state_system.run_if(in_state(UndoState::RequestUndo)))
            .add_systems(PostUpdate, reserve_reset_system.run_if(in_state(UndoState::CommitReservations)));

        #[cfg(feature = "callback_event")]
        app.add_plugins(crate::undo_event::callback::UndoCallbackEventPlugin);
    }
}


#[derive(Resource, Default, Debug)]
pub(crate) struct Posted(bool);


#[derive(States, Default, PartialEq, Debug, Copy, Clone, Eq, Hash)]
enum UndoState {
    #[default]
    None,

    RequestUndo,

    CommitReservations,
}


#[derive(Resource)]
struct UndoStack<T: Event + Clone>(Vec<UndoEvent<T>>);


impl<T: Event + Clone> Default for UndoStack<T> {
    #[inline(always)]
    fn default() -> Self {
        Self(vec![])
    }
}


#[derive(Event)]
struct UndoWaitEvent;


impl<E: Event + Clone> UndoStack<E> {
    #[inline(always)]
    pub fn push(&mut self, e: UndoEvent<E>) {
        self.0.push(e);
    }


    #[inline(always)]
    pub fn pop_if_has_latest(&mut self, counter: &UndoCounter) -> Option<E> {
        let index = self.0.iter().position(|undo| undo.no == **counter)?;
        Some(self.0.remove(index).inner)
    }
}


fn request_undo_system(
    mut reserve_reader: EventReader<RequestCommitReservationsFromSchedulerEvent>,
    mut reserve_reader2: EventReader<RequestCommitReservationsEvent>,
    mut undo_reader: EventReader<RequestUndoEvent>,
    mut wait: EventWriter<UndoWaitEvent>,
    mut state: ResMut<NextState<UndoState>>,
    mut posted: ResMut<Posted>,
) {
    if reserve_reader.iter().next().is_some() || reserve_reader2.iter().next().is_some() {
        state.set(UndoState::CommitReservations);
        if undo_reader.iter().next().is_some() {
            wait.send(UndoWaitEvent);
        }
    } else if undo_reader.iter().next().is_some() {
        posted.0 = false;
        state.set(UndoState::RequestUndo);
    }
}


fn undo_wait_event_system(
    mut er: EventReader<UndoWaitEvent>,
    mut ew: EventWriter<RequestUndoEvent>,
    mut posted: ResMut<Posted>,
) {
    if er.iter().next().is_some() {
        posted.0 = false;
        ew.send(RequestUndoEvent);
    }
}


fn reset_state_system(
    mut state: ResMut<NextState<UndoState>>,
    mut counter: ResMut<UndoCounter>,
    posted: Res<Posted>,
) {
    if posted.0 {
        counter.decrement();
    }

    state.set(UndoState::None);
}


fn reserve_reset_system(
    mut state: ResMut<NextState<UndoState>>,
    mut counter: ResMut<UndoCounter>,
    mut reserve_counter: ResMut<ReserveCounter>,
) {
    *counter += *reserve_counter;
    reserve_counter.reset();
    state.set(UndoState::None);
}