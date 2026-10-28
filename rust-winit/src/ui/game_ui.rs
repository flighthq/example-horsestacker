//! The five-screen game UI, corresponding to `ui/gameUi.ts`.

use std::sync::{Arc, RwLock};

use flight_sdk::{
    DisplayObject, GlRenderState, Node2D, NodeRef, Ref, RichText, Shape, TextFormat,
    TextFormatAlign, TextLabel, add_node_child, create_display_object, create_rich_text,
    create_shape, ease_out_back, ease_out_cubic, invalidate_node_appearance,
    set_rich_text_default_text_format, set_rich_text_multiline, set_rich_text_string,
    set_rich_text_width, set_rich_text_word_wrap, set_text_label_width,
};

use crate::game::GameMode;

use super::{
    GOLD, INK, SANS, SERIF, UiRenderer, create_ui_renderer, draw_fullscreen, draw_speaker, fill,
    fit_label_to_width, label, place, set_text, show,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiScreen {
    Loading,
    Title,
    Playing,
    TimeUp,
    Result,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiButtonId {
    Time,
    Steady,
    Again,
    Menu,
    Fullscreen,
    Credits,
    Mute,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiButton {
    pub height: f32,
    pub id: UiButtonId,
    pub width: f32,
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug)]
pub struct UiModel {
    pub mode: GameMode,
    pub live_height_text: String,
    pub next_piece_glyph: String,
    pub muted: bool,
    pub best_text: String,
    pub credits_open: bool,
    pub count_progress: f64,
    pub credits_text: Option<String>,
    pub hands_shown: i64,
    pub height_text: String,
    pub is_record: bool,
    pub now: f64,
    pub pointer_down: bool,
    pub pointer_x: f32,
    pub pointer_y: f32,
    pub screen: UiScreen,
    pub seconds_left: f64,
    pub time_up_progress: f64,
}

const SCREEN_GUTTER: f32 = 20.0;
const TITLE_MAX_SIZE: f32 = 96.0;
const TITLE_MIN_SIZE: f32 = 30.0;
const TIME_UP_MAX_SIZE: f32 = 112.0;
const TIME_UP_MIN_SIZE: f32 = 34.0;
const TITLE_PIVOT_RATIO: f32 = 60.0 / 96.0;
const MODE_PILL_WIDTH: f32 = 216.0;
const STEADY_PILL_HEIGHT: f32 = 62.0;
const SHOW_NEXT_PREVIEW: bool = false;
const HANDS_PER_EMOJI_COLUMN: usize = 7;
const HANDS_PER_EMOJI: i64 = 2;
const TALLY_CAPACITY: usize = 126;

fn time_up_scale(t: f32) -> f32 {
    if t <= 0.55 {
        1.18 + (0.97 - 1.18) * ease_out_cubic(t / 0.55)
    } else {
        0.97 + (1.0 - 0.97) * ease_out_cubic((t - 0.55) / 0.45)
    }
}

fn slam_curve(t: f32) -> f32 {
    if t <= 0.68 {
        0.58 + (1.16 - 0.58) * ease_out_cubic(t / 0.68)
    } else {
        1.16 + (1.0 - 1.16) * ease_out_cubic((t - 0.68) / 0.32)
    }
}

fn slam_scale(t: f32) -> f32 {
    if t <= 0.0 { 1.0 } else { slam_curve(t) }
}

pub type UiState = GameUi2D;

pub struct GameUi2D {
    renderer: UiRenderer,
    root: Ref<DisplayObject>,
    title_text: Ref<TextLabel>,
    time_pill: Ref<Shape>,
    time_text: Ref<TextLabel>,
    steady_pill: Ref<Shape>,
    steady_text: Ref<TextLabel>,
    time_blurb: Ref<TextLabel>,
    steady_blurb: Ref<TextLabel>,
    menu_pill: Ref<Shape>,
    menu_text: Ref<TextLabel>,
    next_pill: Ref<Shape>,
    next_caption: Ref<TextLabel>,
    next_glyph: Ref<TextLabel>,
    timer_pill: Ref<Shape>,
    timer_caption: Ref<TextLabel>,
    timer_value: Ref<TextLabel>,
    height_pill: Ref<Shape>,
    height_caption: Ref<TextLabel>,
    height_value: Ref<TextLabel>,
    time_up_scrim: Ref<Shape>,
    time_up_text: Ref<TextLabel>,
    tally_horses: Vec<Ref<TextLabel>>,
    horse_appeared_at: Vec<f64>,
    result_height: Ref<TextLabel>,
    record_badge: Ref<DisplayObject>,
    record_pill: Ref<Shape>,
    record_text: Ref<TextLabel>,
    best_label: Ref<TextLabel>,
    again_pill: Ref<Shape>,
    again_text: Ref<TextLabel>,
    menu_from_result_pill: Ref<Shape>,
    menu_from_result_text: Ref<TextLabel>,
    credits_pill: Ref<Shape>,
    credits_text: Ref<TextLabel>,
    credits_body: Ref<Shape>,
    credits_copy: Ref<RichText>,
    fullscreen_pill: Ref<Shape>,
    fullscreen_icon: Ref<Shape>,
    mute_pill: Ref<Shape>,
    mute_icon: Ref<Shape>,
    pub buttons: Vec<UiButton>,
    width: f32,
    height: f32,
    screen_shown_at: f64,
    last_screen: Option<UiScreen>,
    pointer_down: bool,
    pointer_x: f32,
    pointer_y: f32,
    hovering_anything: bool,
}

pub fn create_game_ui_2d(screen_state: &GlRenderState, pixel_ratio: f32) -> GameUi2D {
    let renderer = create_ui_renderer(screen_state, pixel_ratio);
    let root = create_display_object();
    let title_text = label("Horse Stacker", TITLE_MAX_SIZE, GOLD, SERIF, true);
    let time_pill = create_shape();
    let time_text = label("TIME CHALLENGE", 13.0, 0x252420ff, SANS, true);
    let steady_pill = create_shape();
    let steady_text = label("STEADY HANDS", 13.0, 0x252420ff, SANS, true);
    let time_blurb = label(
        "Stack as high as you can in 30 seconds",
        11.0,
        INK,
        SANS,
        false,
    );
    let steady_blurb = label("No time but no horse may fall off", 11.0, INK, SANS, false);
    let menu_pill = create_shape();
    let menu_text = label("MENU", 12.0, INK, SANS, true);
    let next_pill = create_shape();
    let next_caption = label("NEXT", 8.0, 0xd8e0d2ff, SANS, true);
    let next_glyph = label("", 20.0, INK, SANS, false);
    let timer_pill = create_shape();
    let timer_caption = label("TIME LEFT", 9.0, 0xd8e0d2ff, SANS, true);
    let timer_value = label("30", 34.0, INK, SERIF, false);
    let height_pill = create_shape();
    let height_caption = label("STACK HEIGHT", 9.0, 0xd8e0d2ff, SANS, true);
    let height_value = label("0.00 m", 22.0, INK, SERIF, false);
    let time_up_scrim = create_shape();
    let time_up_text = label("TIME UP!", TIME_UP_MAX_SIZE, GOLD, SERIF, true);
    let tally_horses = (0..TALLY_CAPACITY)
        .map(|_| label("🐴", 15.0, INK, SANS, false))
        .collect::<Vec<_>>();
    let result_height = label("0.00 m", 74.0, GOLD, SERIF, false);
    let record_badge = create_display_object();
    let record_pill = create_shape();
    let record_text = label("NEW RECORD!", 12.0, 0x3a2c07ff, SANS, true);
    add_child(&record_badge, &record_pill);
    add_child(&record_badge, &record_text);
    let best_label = label("", 11.0, GOLD, SANS, true);
    let again_pill = create_shape();
    let again_text = label("PLAY AGAIN", 13.0, 0x252420ff, SANS, true);
    let menu_from_result_pill = create_shape();
    let menu_from_result_text = label("MENU", 13.0, INK, SANS, true);
    let credits_pill = create_shape();
    let credits_text = label("i", 15.0, INK, SERIF, false);
    let credits_body = create_shape();
    let credits_copy = create_rich_text(None);
    set_rich_text_multiline(&credits_copy, true);
    set_rich_text_word_wrap(&credits_copy, true);
    set_rich_text_default_text_format(
        &credits_copy,
        Arc::new(RwLock::new(TextFormat {
            align: Some(TextFormatAlign::Left),
            color: Some(0xd8e0d2ff),
            font: Some(SANS.to_owned()),
            leading: Some(3.0),
            size: Some(11.0),
            ..TextFormat::default()
        })),
    );
    let fullscreen_pill = create_shape();
    let fullscreen_icon = create_shape();
    let mute_pill = create_shape();
    let mute_icon = create_shape();

    for node in [
        erased(&title_text),
        erased(&time_pill),
        erased(&time_text),
        erased(&steady_pill),
        erased(&steady_text),
        erased(&time_blurb),
        erased(&steady_blurb),
        erased(&timer_pill),
        erased(&timer_caption),
        erased(&timer_value),
        erased(&height_pill),
        erased(&height_caption),
        erased(&height_value),
        erased(&menu_pill),
        erased(&menu_text),
        erased(&next_pill),
        erased(&next_caption),
        erased(&next_glyph),
        erased(&time_up_scrim),
        erased(&time_up_text),
    ] {
        let parent: NodeRef = root.clone();
        add_node_child(&parent, &node);
    }
    for horse in &tally_horses {
        add_child(&root, horse);
    }
    for node in [
        erased(&result_height),
        erased(&record_badge),
        erased(&best_label),
        erased(&again_pill),
        erased(&again_text),
        erased(&credits_body),
        erased(&credits_copy),
        erased(&credits_pill),
        erased(&credits_text),
        erased(&fullscreen_pill),
        erased(&fullscreen_icon),
        erased(&mute_pill),
        erased(&mute_icon),
        erased(&menu_from_result_pill),
        erased(&menu_from_result_text),
    ] {
        let parent: NodeRef = root.clone();
        add_node_child(&parent, &node);
    }

    GameUi2D {
        renderer,
        root,
        title_text,
        time_pill,
        time_text,
        steady_pill,
        steady_text,
        time_blurb,
        steady_blurb,
        menu_pill,
        menu_text,
        next_pill,
        next_caption,
        next_glyph,
        timer_pill,
        timer_caption,
        timer_value,
        height_pill,
        height_caption,
        height_value,
        time_up_scrim,
        time_up_text,
        tally_horses,
        horse_appeared_at: vec![0.0; TALLY_CAPACITY],
        result_height,
        record_badge,
        record_pill,
        record_text,
        best_label,
        again_pill,
        again_text,
        menu_from_result_pill,
        menu_from_result_text,
        credits_pill,
        credits_text,
        credits_body,
        credits_copy,
        fullscreen_pill,
        fullscreen_icon,
        mute_pill,
        mute_icon,
        buttons: Vec::new(),
        width: 1.0,
        height: 1.0,
        screen_shown_at: 0.0,
        last_screen: None,
        pointer_down: false,
        pointer_x: -1.0,
        pointer_y: -1.0,
        hovering_anything: false,
    }
}

impl GameUi2D {
    pub fn resize(&mut self, width: f32, height: f32, pixel_ratio: f32) -> Result<(), String> {
        self.width = width.round().max(1.0);
        self.height = height.round().max(1.0);
        self.renderer.resize(self.width, self.height, pixel_ratio)
    }

    pub fn render(&mut self) -> Result<(), String> {
        let root: NodeRef = self.root.clone();
        self.renderer.render(&root)
    }

    pub fn update(&mut self, model: &UiModel) -> bool {
        let was_hovering = self.hovering_anything;
        self.hovering_anything = false;
        self.pointer_down = model.pointer_down;
        self.pointer_x = model.pointer_x;
        self.pointer_y = model.pointer_y;
        if self.last_screen != Some(model.screen) {
            self.last_screen = Some(model.screen);
            self.screen_shown_at = model.now;
        }
        let intro = saturate(((model.now - self.screen_shown_at) / 380.0) as f32);
        let pop = ease_out_back(intro);
        self.buttons.clear();
        let on_title = model.screen == UiScreen::Title;
        let on_result = model.screen == UiScreen::Result;
        let on_time_up = model.screen == UiScreen::TimeUp;
        let playing = model.screen == UiScreen::Playing;

        for node in [
            &self.title_text,
            &self.time_text,
            &self.steady_text,
            &self.time_blurb,
            &self.steady_blurb,
        ] {
            show(node, on_title);
        }
        show(&self.time_pill, on_title);
        show(&self.steady_pill, on_title);
        if on_title {
            self.layout_title(model, intro, pop);
        }

        show(&self.time_up_scrim, on_time_up);
        show(&self.time_up_text, on_time_up);
        if on_time_up {
            self.layout_time_up(model);
        }

        let show_again = on_result && model.count_progress >= 1.0;
        let show_record = on_result && model.count_progress >= 0.86;
        show(&self.result_height, on_result);
        show(&self.record_badge, show_record && model.is_record);
        show(&self.best_label, show_record && !model.best_text.is_empty());
        show(&self.again_pill, show_again);
        show(&self.again_text, show_again);
        show(&self.menu_from_result_pill, show_again);
        show(&self.menu_from_result_text, show_again);
        if !on_result {
            for horse in &self.tally_horses {
                show(horse, false);
            }
        } else {
            self.layout_result(model, show_record);
        }

        let timed_playing = playing && model.mode == GameMode::Time;
        let steady_playing = playing && model.mode == GameMode::Steady;
        show(&self.timer_pill, timed_playing);
        show(&self.timer_caption, timed_playing);
        show(&self.timer_value, timed_playing);
        if timed_playing {
            self.layout_timed_hud(model, pop);
        }
        show(&self.height_pill, steady_playing);
        show(&self.height_caption, steady_playing);
        show(&self.height_value, steady_playing);
        show(&self.menu_pill, steady_playing);
        show(&self.menu_text, steady_playing);
        if steady_playing {
            self.layout_steady_hud(model, pop);
        }

        let show_next = SHOW_NEXT_PREVIEW && playing;
        show(&self.next_pill, show_next);
        show(&self.next_caption, show_next);
        show(&self.next_glyph, show_next);
        if show_next {
            self.layout_next(model, pop, steady_playing);
        }

        show(&self.credits_pill, on_result);
        show(&self.credits_text, on_result);
        if on_result {
            let pointer = self.pointer();
            self.hovering_anything |= pill(
                pointer,
                &mut self.buttons,
                &self.credits_pill,
                &self.credits_text,
                UiButtonId::Credits,
                24.0,
                self.height - 54.0,
                30.0,
                30.0,
                0x1f2d1dff,
                0.42,
            );
        }
        let pointer = self.pointer();
        self.hovering_anything |= icon_pill(
            pointer,
            &mut self.buttons,
            &self.fullscreen_pill,
            &self.fullscreen_icon,
            UiButtonId::Fullscreen,
            self.width - 54.0,
            self.height - 54.0,
            30.0,
            0x1f2d1dff,
            0.42,
            |shape, colour, alpha| draw_fullscreen(shape, colour, alpha),
        );
        self.hovering_anything |= icon_pill(
            pointer,
            &mut self.buttons,
            &self.mute_pill,
            &self.mute_icon,
            UiButtonId::Mute,
            self.width - 92.0,
            self.height - 54.0,
            30.0,
            0x1f2d1dff,
            0.42,
            |shape, colour, alpha| draw_speaker(shape, colour, alpha, model.muted),
        );
        self.layout_credits(model, on_result);

        let settling = model.screen == UiScreen::Result && model.count_progress < 1.0;
        let arriving = intro < 1.0;
        let idle_motion = model.screen == UiScreen::Title
            || model.screen == UiScreen::Playing
            || (model.screen == UiScreen::Result && model.is_record);
        arriving || settling || idle_motion || self.hovering_anything || was_hovering
    }

    fn pointer(&self) -> PointerState {
        PointerState {
            down: self.pointer_down,
            x: self.pointer_x,
            y: self.pointer_y,
        }
    }

    fn layout_title(&mut self, model: &UiModel, intro: f32, pop: f32) {
        let breathe = (model.now * 0.0016).sin() as f32 * 4.0;
        set_text_label_width(&self.title_text, self.width);
        let title_size = fit_label_to_width(
            &self.title_text,
            TITLE_MAX_SIZE,
            TITLE_MIN_SIZE,
            (self.width - SCREEN_GUTTER * 2.0) / 1.02,
        );
        mutate_node(&self.title_text, |node| {
            node.alpha = intro;
            node.scale_x = 0.86 + 0.14 * pop;
            node.scale_y = node.scale_x;
            node.pivot_x = self.width / 2.0;
            node.pivot_y = title_size * TITLE_PIVOT_RATIO;
        });
        place(
            &self.title_text,
            self.width / 2.0,
            self.height * 0.5 - 120.0
                + title_size * TITLE_PIVOT_RATIO
                + (1.0 - pop) * 26.0
                + breathe,
        );
        let nudge = (model.now * 0.0016 + 1.1).sin() as f32 * 2.0;
        let menu_width = MODE_PILL_WIDTH.min(self.width - SCREEN_GUTTER * 2.0);
        let menu_x = self.width / 2.0 - menu_width / 2.0;
        let menu_y = self.height * 0.5 + 24.0 + (1.0 - intro) * 20.0 + nudge;
        let blurb_width = (self.width - SCREEN_GUTTER * 2.0).min(MODE_PILL_WIDTH + 150.0);
        let blurb_x = self.width / 2.0 - blurb_width / 2.0;
        let pointer = self.pointer();
        self.hovering_anything |= pill(
            pointer,
            &mut self.buttons,
            &self.time_pill,
            &self.time_text,
            UiButtonId::Time,
            menu_x,
            menu_y,
            menu_width,
            44.0,
            INK,
            intro,
        );
        set_text_label_width(&self.time_blurb, blurb_width);
        mutate_node(&self.time_blurb, |node| node.alpha = intro * 0.72);
        place(&self.time_blurb, blurb_x, menu_y + 48.0);
        self.hovering_anything |= pill(
            pointer,
            &mut self.buttons,
            &self.steady_pill,
            &self.steady_text,
            UiButtonId::Steady,
            menu_x,
            menu_y + 82.0,
            menu_width,
            44.0,
            INK,
            intro,
        );
        set_text_label_width(&self.steady_blurb, blurb_width);
        mutate_node(&self.steady_blurb, |node| node.alpha = intro * 0.72);
        place(&self.steady_blurb, blurb_x, menu_y + 130.0);
    }

    fn layout_time_up(&mut self, model: &UiModel) {
        let progress = model.time_up_progress.clamp(0.0, 1.0) as f32;
        let scale = time_up_scale(progress);
        fill(
            &self.time_up_scrim,
            0x7e311fff,
            0.88 * (progress * 3.0).min(1.0) as f64,
            0.0,
            0.0,
            self.width as f64,
            self.height as f64,
            0.0,
        );
        place(&self.time_up_scrim, 0.0, 0.0);
        set_text_label_width(&self.time_up_text, self.width);
        let size = fit_label_to_width(
            &self.time_up_text,
            TIME_UP_MAX_SIZE,
            TIME_UP_MIN_SIZE,
            (self.width - SCREEN_GUTTER * 2.0) / 1.2,
        );
        mutate_node(&self.time_up_text, |node| {
            node.scale_x = scale;
            node.scale_y = scale;
            node.alpha = (progress * 2.4).min(1.0);
            node.pivot_x = self.width / 2.0;
            node.pivot_y = size * TITLE_PIVOT_RATIO;
            node.rotation = -0.052;
        });
        place(&self.time_up_text, self.width / 2.0, self.height * 0.5);
    }

    fn layout_result(&mut self, model: &UiModel, show_record: bool) {
        let rule_y = self.height * 0.5 - 42.0;
        let shown = (model.hands_shown / HANDS_PER_EMOJI)
            .max(0)
            .min(TALLY_CAPACITY as i64) as usize;
        let columns = 1usize.max(shown.div_ceil(HANDS_PER_EMOJI_COLUMN));
        let start_x = self.width / 2.0 - columns as f32 * 19.0 / 2.0;
        for (index, horse) in self.tally_horses.iter().enumerate() {
            if index >= shown {
                show(horse, false);
                self.horse_appeared_at[index] = 0.0;
                continue;
            }
            if self.horse_appeared_at[index] == 0.0 {
                self.horse_appeared_at[index] = model.now;
            }
            let column = index / HANDS_PER_EMOJI_COLUMN;
            let row = index % HANDS_PER_EMOJI_COLUMN;
            let age = ((model.now - self.horse_appeared_at[index]) / 340.0).clamp(0.0, 1.0) as f32;
            let pop = if age < 0.7 {
                0.2 + 0.98 * ease_out_cubic(age / 0.7)
            } else {
                1.18 - 0.18 * ease_out_cubic((age - 0.7) / 0.3)
            };
            show(horse, true);
            set_text_label_width(horse, 19.0);
            mutate_node(horse, |node| {
                node.scale_x = pop;
                node.scale_y = pop;
                node.alpha = (age * 3.0).min(1.0);
            });
            place(
                horse,
                start_x + column as f32 * 19.0,
                rule_y - (row + 1) as f32 * 17.0 - 4.0 + (1.0 - age) * 18.0,
            );
        }
        let settle = saturate(((model.count_progress - 0.86) / 0.14) as f32);
        let slam = slam_scale(settle);
        set_text_label_width(&self.result_height, self.width);
        set_text(&self.result_height, &model.height_text);
        mutate_node(&self.result_height, |node| {
            node.scale_x = slam;
            node.scale_y = slam;
            node.pivot_x = self.width / 2.0;
            node.pivot_y = 40.0;
        });
        place(&self.result_height, self.width / 2.0, rule_y + 52.0);

        let badge_width = 152.0;
        let badge_height = 26.0;
        let badge_y = rule_y + 98.0;
        let badge_showing = show_record && model.is_record;
        if badge_showing {
            let swagger = (model.now * 0.005).sin() as f32;
            fill(
                &self.record_pill,
                GOLD,
                (0.9 + swagger * 0.1) as f64,
                0.0,
                0.0,
                badge_width as f64,
                badge_height as f64,
                (badge_height / 2.0) as f64,
            );
            place(&self.record_pill, 0.0, 0.0);
            set_text_label_width(&self.record_text, badge_width);
            place(&self.record_text, 0.0, (badge_height - 12.0 * 1.35) / 2.0);
            let arrive = ease_out_back(settle);
            mutate_node(&self.record_badge, |node| {
                node.scale_x = arrive;
                node.scale_y = arrive;
                node.pivot_x = badge_width / 2.0;
                node.pivot_y = badge_height / 2.0;
                node.alpha = saturate(settle * 2.2);
            });
            place(
                &self.record_badge,
                self.width / 2.0,
                badge_y + badge_height / 2.0 + (swagger * 2.0).round(),
            );
        }
        if show_record && !model.best_text.is_empty() {
            set_text_label_width(&self.best_label, self.width);
            set_text(&self.best_label, &model.best_text);
            mutate_node(&self.best_label, |node| node.alpha = saturate(settle * 1.6));
            place(
                &self.best_label,
                0.0,
                badge_y + 8.0 + (1.0 - ease_out_cubic(settle)) * 10.0,
            );
        }
        let buttons_y = rule_y
            + if badge_showing { 160.0 } else { 150.0 }
            + (1.0 - ease_out_back(settle)) * 16.0;
        let again_width = 192.0f32.min((self.width - SCREEN_GUTTER * 2.0 - 108.0).max(120.0));
        let menu_width = 96.0;
        let pair_x = self.width / 2.0 - (again_width + 8.0 + menu_width) / 2.0;
        let pointer = self.pointer();
        self.hovering_anything |= pill(
            pointer,
            &mut self.buttons,
            &self.again_pill,
            &self.again_text,
            UiButtonId::Again,
            pair_x,
            buttons_y,
            again_width,
            44.0,
            INK,
            settle,
        );
        self.hovering_anything |= pill(
            pointer,
            &mut self.buttons,
            &self.menu_from_result_pill,
            &self.menu_from_result_text,
            UiButtonId::Menu,
            pair_x + again_width + 8.0,
            buttons_y,
            menu_width,
            44.0,
            0x1f2d1dff,
            settle * 0.82,
        );
    }

    fn layout_timed_hud(&mut self, model: &UiModel, pop: f32) {
        let width = 104.0;
        let x = self.width - width - 24.0;
        let drop = (1.0 - pop) * 40.0;
        let urgent = saturate(((10.0 - model.seconds_left) / 4.0) as f32);
        let fraction = model.seconds_left - model.seconds_left.floor();
        let kick = (1.0 - (1.0 - fraction) * 7.0).max(0.0) as f32;
        let shake = urgent * (model.now * 0.045).sin() as f32 * 2.5;
        fill(
            &self.timer_pill,
            if urgent > 0.0 { 0x8c3a24ff } else { 0x1f2d1dff },
            (0.72 + urgent * 0.14) as f64,
            0.0,
            0.0,
            width as f64,
            62.0,
            18.0,
        );
        place(&self.timer_pill, x + shake, 18.0 - drop);
        set_text_label_width(&self.timer_caption, width);
        place(&self.timer_caption, x + shake, 26.0 - drop);
        set_text_label_width(&self.timer_value, width);
        let beat = 1.0 + kick * (0.14 + urgent * 0.12);
        mutate_node(&self.timer_value, |node| {
            node.scale_x = beat;
            node.scale_y = beat;
            node.pivot_x = width / 2.0;
            node.pivot_y = 20.0;
        });
        place(&self.timer_value, x + shake + width / 2.0, 58.0 - drop);
        set_text(
            &self.timer_value,
            &model.seconds_left.max(0.0).ceil().to_string(),
        );
    }

    fn layout_steady_hud(&mut self, model: &UiModel, pop: f32) {
        let width = 104.0;
        let x = self.width - width - 24.0;
        let drop = (1.0 - pop) * 40.0;
        fill(
            &self.height_pill,
            0x1f2d1dff,
            0.72,
            0.0,
            0.0,
            width as f64,
            STEADY_PILL_HEIGHT as f64,
            18.0,
        );
        place(&self.height_pill, x, 18.0 - drop);
        set_text_label_width(&self.height_caption, width);
        place(&self.height_caption, x, 26.0 - drop);
        set_text_label_width(&self.height_value, width);
        place(&self.height_value, x, 44.0 - drop);
        set_text(&self.height_value, &model.live_height_text);
        let pointer = self.pointer();
        self.hovering_anything |= pill(
            pointer,
            &mut self.buttons,
            &self.menu_pill,
            &self.menu_text,
            UiButtonId::Menu,
            24.0,
            18.0 - drop,
            84.0,
            32.0,
            0x1f2d1dff,
            0.62,
        );
    }

    fn layout_next(&mut self, model: &UiModel, pop: f32, steady_playing: bool) {
        let width = 62.0;
        let x = self.width - 24.0 - width;
        let drop = (1.0 - pop) * 40.0;
        let y =
            18.0 + if steady_playing {
                STEADY_PILL_HEIGHT
            } else {
                62.0
            } + 8.0
                - drop;
        fill(
            &self.next_pill,
            0x1f2d1dff,
            0.62,
            0.0,
            0.0,
            width as f64,
            42.0,
            14.0,
        );
        place(&self.next_pill, x, y);
        set_text_label_width(&self.next_caption, width);
        place(&self.next_caption, x, y + 6.0);
        set_text_label_width(&self.next_glyph, width);
        place(&self.next_glyph, x, y + 15.0);
        set_text(&self.next_glyph, &model.next_piece_glyph);
    }

    fn layout_credits(&mut self, model: &UiModel, on_result: bool) {
        let showing = on_result && model.credits_open;
        show(&self.credits_body, showing);
        show(&self.credits_copy, showing);
        if !showing {
            return;
        }
        let width = 420.0f32.min(self.width - 48.0);
        fill(
            &self.credits_body,
            0x182217ff,
            0.9,
            0.0,
            0.0,
            width as f64,
            82.0,
            16.0,
        );
        place(&self.credits_body, 24.0, self.height - 148.0);
        set_rich_text_width(&self.credits_copy, width - 32.0);
        set_rich_text_string(
            &self.credits_copy,
            model.credits_text.as_deref().unwrap_or(
                "Built with Flight · Models by EdwinRC and SleepyPineapple, CC BY 4.0 · Music The Mountain's Happy Song by Elijah_K via Free Music Archive, CC BY · Ambience and effects via Free Sound Effects",
            ),
        );
        place(&self.credits_copy, 40.0, self.height - 134.0);
    }
}

#[derive(Clone, Copy)]
struct PointerState {
    down: bool,
    x: f32,
    y: f32,
}

fn hovering(pointer: PointerState, x: f32, y: f32, width: f32, height: f32) -> bool {
    pointer.x >= x && pointer.x <= x + width && pointer.y >= y && pointer.y <= y + height
}

#[allow(clippy::too_many_arguments)]
fn pill(
    pointer: PointerState,
    buttons: &mut Vec<UiButton>,
    shape: &Ref<Shape>,
    text: &Ref<TextLabel>,
    id: UiButtonId,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    colour: u32,
    alpha: f32,
) -> bool {
    let hovered = hovering(pointer, x, y, width, height);
    let pressed = hovered && pointer.down;
    let grow = if pressed {
        -1.5
    } else if hovered {
        2.5
    } else {
        0.0
    };
    let sink = if pressed { 1.5 } else { 0.0 };
    fill(
        shape,
        colour,
        (alpha + if hovered { 0.14 } else { 0.0 }).min(1.0) as f64,
        -grow as f64,
        -grow as f64,
        (width + grow * 2.0) as f64,
        (height + grow * 2.0) as f64,
        ((height + grow * 2.0) / 2.0) as f64,
    );
    place(shape, x, y + sink);
    set_text_label_width(text, width);
    let size = text
        .read()
        .unwrap()
        .data
        .text_format
        .read()
        .unwrap()
        .size
        .unwrap_or(12.0);
    place(text, x, y + sink + (height - size * 1.35) / 2.0);
    buttons.push(UiButton {
        height,
        id,
        width,
        x,
        y,
    });
    hovered
}

#[allow(clippy::too_many_arguments)]
fn icon_pill(
    pointer: PointerState,
    buttons: &mut Vec<UiButton>,
    shape: &Ref<Shape>,
    icon: &Ref<Shape>,
    id: UiButtonId,
    x: f32,
    y: f32,
    size: f32,
    colour: u32,
    alpha: f32,
    draw: impl Fn(&Ref<Shape>, u32, f64),
) -> bool {
    let hovered = hovering(pointer, x, y, size, size);
    let pressed = hovered && pointer.down;
    let grow = if pressed {
        -1.5
    } else if hovered {
        2.5
    } else {
        0.0
    };
    let sink = if pressed { 1.5 } else { 0.0 };
    fill(
        shape,
        colour,
        (alpha + if hovered { 0.14 } else { 0.0 }).min(1.0) as f64,
        -grow as f64,
        -grow as f64,
        (size + grow * 2.0) as f64,
        (size + grow * 2.0) as f64,
        ((size + grow * 2.0) / 2.0) as f64,
    );
    place(shape, x, y + sink);
    draw(icon, INK, 1.0);
    place(icon, x + size / 2.0, y + sink + size / 2.0);
    buttons.push(UiButton {
        height: size,
        id,
        width: size,
        x,
        y,
    });
    hovered
}

fn mutate_node<T: Node2D + 'static>(
    node: &Ref<T>,
    mutate: impl FnOnce(&mut flight_sdk::Node2DFields),
) {
    mutate(node.write().unwrap().node2d_fields_mut());
    let node: NodeRef = node.clone();
    invalidate_node_appearance(&node);
}

fn erased<T: Node2D + 'static>(node: &Ref<T>) -> NodeRef {
    node.clone()
}

fn add_child<P: Node2D + 'static, C: Node2D + 'static>(parent: &Ref<P>, child: &Ref<C>) {
    let parent: NodeRef = parent.clone();
    let child: NodeRef = child.clone();
    add_node_child(&parent, &child);
}

fn saturate(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}
