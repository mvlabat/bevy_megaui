use crate::{MegaUiContext, MegaUiSettings, WindowSize};
use bevy::{
    app::Events,
    ecs::{Resources, World},
    input::{keyboard::KeyCode, mouse::MouseButton, Input},
    window::{CursorMoved, ReceivedCharacter, Windows},
};

// Is a thread local system, because `megaui::Ui` (`MegaUiContext`) doesn't implement Send + Sync.
pub fn process_input(_world: &mut World, resources: &mut Resources) {
    use megaui::InputHandler;

    let mut ctx = resources.get_thread_local_mut::<MegaUiContext>().unwrap();
    let ev_cursor = resources.get::<Events<CursorMoved>>().unwrap();
    let ev_received_character = resources.get::<Events<ReceivedCharacter>>().unwrap();
    let mouse_button_input = resources.get::<Input<MouseButton>>().unwrap();
    let keyboard_input = resources.get::<Input<KeyCode>>().unwrap();
    let mut window_size = resources.get_mut::<WindowSize>().unwrap();
    let windows = resources.get::<Windows>().unwrap();
    let megaui_settings = resources.get::<MegaUiSettings>().unwrap();

    if let Some(window) = windows.get_primary() {
        *window_size = WindowSize::new(
            window.physical_width() as f32,
            window.physical_height() as f32,
            window.scale_factor() as f32,
        );
    }

    if let Some(cursor_moved) = ctx.cursor.latest(&ev_cursor) {
        if cursor_moved.id.is_primary() {
            let scale_factor = megaui_settings.scale_factor as f32;
            let mut mouse_position: (f32, f32) = (cursor_moved.position / scale_factor).into();
            mouse_position.1 = window_size.height() / scale_factor - mouse_position.1;
            ctx.mouse_position = mouse_position;
            ctx.ui.mouse_move(mouse_position);
        }
    }

    let mouse_position = ctx.mouse_position;
    if mouse_button_input.just_pressed(MouseButton::Left) {
        ctx.ui.mouse_down(mouse_position);
    }
    if mouse_button_input.just_released(MouseButton::Left) {
        ctx.ui.mouse_up(mouse_position);
    }

    let shift = keyboard_input.pressed(KeyCode::LShift) || keyboard_input.pressed(KeyCode::RShift);
    let ctrl =
        keyboard_input.pressed(KeyCode::LControl) || keyboard_input.pressed(KeyCode::RControl);

    for event in ctx.received_character.iter(&ev_received_character) {
        if event.id.is_primary() && !event.char.is_control() {
            ctx.ui.char_event(event.char, shift, ctrl);
        }
    }

    if keyboard_input.pressed(KeyCode::Up) {
        ctx.ui.key_down(megaui::KeyCode::Up, shift, ctrl);
    }
    if keyboard_input.pressed(KeyCode::Down) {
        ctx.ui.key_down(megaui::KeyCode::Down, shift, ctrl);
    }
    if keyboard_input.pressed(KeyCode::Right) {
        ctx.ui.key_down(megaui::KeyCode::Right, shift, ctrl);
    }
    if keyboard_input.pressed(KeyCode::Left) {
        ctx.ui.key_down(megaui::KeyCode::Left, shift, ctrl);
    }
    if keyboard_input.pressed(KeyCode::Home) {
        ctx.ui.key_down(megaui::KeyCode::Home, shift, ctrl);
    }
    if keyboard_input.pressed(KeyCode::End) {
        ctx.ui.key_down(megaui::KeyCode::End, shift, ctrl);
    }
    if keyboard_input.pressed(KeyCode::Delete) {
        ctx.ui.key_down(megaui::KeyCode::Delete, shift, ctrl);
    }
    if keyboard_input.pressed(KeyCode::Back) {
        ctx.ui.key_down(megaui::KeyCode::Backspace, shift, ctrl);
    }
    if keyboard_input.pressed(KeyCode::Return) {
        ctx.ui.key_down(megaui::KeyCode::Enter, shift, ctrl);
    }
    if keyboard_input.pressed(KeyCode::Tab) {
        ctx.ui.key_down(megaui::KeyCode::Tab, shift, ctrl);
    }
    if keyboard_input.pressed(KeyCode::Z) {
        ctx.ui.key_down(megaui::KeyCode::Z, shift, ctrl);
    }
    if keyboard_input.pressed(KeyCode::Y) {
        ctx.ui.key_down(megaui::KeyCode::Y, shift, ctrl);
    }
    if keyboard_input.pressed(KeyCode::C) {
        ctx.ui.key_down(megaui::KeyCode::C, shift, ctrl);
    }
    if keyboard_input.pressed(KeyCode::X) {
        ctx.ui.key_down(megaui::KeyCode::X, shift, ctrl);
    }
    if keyboard_input.pressed(KeyCode::V) {
        ctx.ui.key_down(megaui::KeyCode::V, shift, ctrl);
    }
    if keyboard_input.pressed(KeyCode::A) {
        ctx.ui.key_down(megaui::KeyCode::A, shift, ctrl);
    }
}
