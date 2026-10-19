use macroquad::audio::{load_sound_from_bytes, play_sound, PlaySoundParams, Sound};
use macroquad::prelude::*;
use slime_cave::game::Game;
use slime_cave::gfx::{Frame, H, W};
use slime_cave::player::Input;
use slime_cave::sound;

fn conf() -> Conf {
    Conf {
        window_title: "Slime Cave".to_owned(),
        window_width: W * 2,
        window_height: H * 2,
        window_resizable: true,
        ..Default::default()
    }
}

fn down(keys: &[KeyCode]) -> bool {
    keys.iter().any(|k| is_key_down(*k))
}

fn pressed(keys: &[KeyCode]) -> bool {
    keys.iter().any(|k| is_key_pressed(*k))
}

/// Held keys, plus anything pressed since the last simulation step so that
/// quick taps are never lost between fixed updates.
type Binding = (&'static [KeyCode], fn(&mut Input) -> &mut bool);

fn read_input(latched: &mut Input) -> Input {
    use KeyCode::*;
    let groups: [Binding; 11] = [
        (&[A, Left], |i| &mut i.left),
        (&[D, Right], |i| &mut i.right),
        (&[W, Up], |i| &mut i.up),
        (&[S, Down], |i| &mut i.down),
        (&[Space, W, Up, Z], |i| &mut i.jump),
        (&[LeftShift, RightShift, X], |i| &mut i.jet),
        (&[J, C], |i| &mut i.blade),
        (&[K, V], |i| &mut i.sonic),
        (&[L, B], |i| &mut i.thread),
        (&[E, Q], |i| &mut i.predate),
        (&[Enter, Escape], |i| &mut i.start),
    ];
    let mut inp = Input::default();
    for (keys, field) in groups {
        if pressed(keys) {
            *field(latched) = true;
        }
        *field(&mut inp) = down(keys) || *field(latched);
    }
    inp
}

#[macroquad::main(conf)]
async fn main() {
    let mut sounds: Vec<(slime_cave::game::Sfx, Sound)> = Vec::new();
    for s in sound::ALL {
        if let Ok(snd) = load_sound_from_bytes(&sound::wav(s)).await {
            sounds.push((s, snd));
        }
    }
    if let Ok(music) = load_sound_from_bytes(&sound::music()).await {
        play_sound(&music, PlaySoundParams { looped: true, volume: 0.35 });
    }
    let mut game = Game::new();
    let mut frame = Frame::new();
    let mut rgba = Vec::new();
    let tex = Texture2D::from_rgba8(W as u16, H as u16, &vec![0; (W * H * 4) as usize]);
    tex.set_filter(FilterMode::Nearest);
    let mut acc = 0.0f32;
    let mut latched = Input::default();
    let step = 1.0 / 60.0;
    loop {
        if is_key_pressed(KeyCode::F11) {
            set_fullscreen(true);
        }
        acc += get_frame_time().min(0.1);
        let inp = read_input(&mut latched);
        while acc >= step {
            game.update(inp);
            latched = Input::default();
            acc -= step;
        }
        let mut played = Vec::new();
        for s in game.sfx.drain(..) {
            if played.contains(&s) {
                continue;
            }
            played.push(s);
            if let Some((_, snd)) = sounds.iter().find(|(k, _)| *k == s) {
                play_sound(snd, PlaySoundParams { looped: false, volume: 0.6 });
            }
        }
        game.render(&mut frame);
        frame.to_rgba(&mut rgba);
        tex.update_from_bytes(W as u32, H as u32, &rgba);
        clear_background(BLACK);
        let s = (screen_width() / W as f32).min(screen_height() / H as f32);
        let (dw, dh) = (W as f32 * s, H as f32 * s);
        draw_texture_ex(
            &tex,
            (screen_width() - dw) / 2.0,
            (screen_height() - dh) / 2.0,
            WHITE,
            DrawTextureParams { dest_size: Some(vec2(dw, dh)), ..Default::default() },
        );
        next_frame().await
    }
}
