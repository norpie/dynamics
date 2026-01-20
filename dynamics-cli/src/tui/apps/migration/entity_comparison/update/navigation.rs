use super::super::app::State;
use super::super::{ActiveTab, Msg};
use crate::tui::command::{AppId, Command};

pub fn handle_back(state: &mut State) -> Command<Msg> {
    state.show_back_confirmation = true;
    Command::None
}

pub fn handle_confirm_back() -> Command<Msg> {
    Command::navigate_to(AppId::MigrationComparisonSelect)
}

pub fn handle_cancel_back(state: &mut State) -> Command<Msg> {
    state.show_back_confirmation = false;
    Command::None
}

pub fn handle_switch_tab(state: &mut State, n: usize) -> Command<Msg> {
    if let Some(tab) = ActiveTab::from_number(n) {
        state.active_tab = tab;
    }
    Command::None
}
