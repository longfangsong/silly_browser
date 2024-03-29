#[macro_use]
extern crate conrod_core;
#[macro_use]
extern crate html5ever;
#[macro_use]
extern crate lazy_static;

use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net;
use std::net::Shutdown;

use conrod_core::{Borderable, Colorable, Labelable, Positionable, Sizeable, Ui, UiCell, widget, Widget};
use conrod_core::text::Font;
use conrod_core::widget::text_box::Event::Update;
use conrod_glium::Renderer;
use glium::Surface;
use html5ever::parse_document;
use html5ever::rcdom::{Handle, NodeData, RcDom};
use html5ever::tendril::TendrilSink;

use crate::parser::parse;
use crate::support::{EventLoop, GliumDisplayWinitWrapper};

mod parser;
mod support;
widget_ids!(struct Ids {
        canvas,
        url_input_area,
        url_input,
        visit_button,
        html_area,
        url,
        elements[]
    });

struct Context {
    pub light_font: conrod_core::text::font::Id,
    pub bold_font: conrod_core::text::font::Id,
    pub current_url_input: String,
    pub dom_string: String,
}

lazy_static! {
    static ref HEADER_SIZE: HashMap<u8, f64> = {
        let mut m = HashMap::new();
        m.insert(1u8, 68.0);
        m.insert(2u8, 60.0);
        m.insert(3u8, 52.0);
        m.insert(4u8, 36.0);
        m.insert(5u8, 28.0);
        m
    };
}

fn main() {
    const WIDTH: u32 = 800;
    const HEIGHT: u32 = 600;
    let mut events_loop = glium::glutin::EventsLoop::new();
    let window = glium::glutin::WindowBuilder::new()
        .with_title("Silly Browser")
        .with_dimensions((WIDTH, HEIGHT).into());
    let context = glium::glutin::ContextBuilder::new()
        .with_vsync(true)
        .with_multisampling(4);
    let display = glium::Display::new(window, context, &events_loop).unwrap();
    let display = support::GliumDisplayWinitWrapper(display);
    let mut event_loop = support::EventLoop::new();
    let mut ui = conrod_core::UiBuilder::new([WIDTH as f64, HEIGHT as f64]).build();
    let (light_font, bold_font) = init_font(&mut ui);
    let mut renderer = conrod_glium::Renderer::new(&display.0).unwrap();
    let image_map = conrod_core::image::Map::<glium::texture::Texture2d>::new();

    let mut ids = Ids::new(ui.widget_id_generator());

    let mut state = Context {
        current_url_input: String::new(),
        light_font,
        bold_font,
        dom_string: include_str!("../example.html").to_string(),
    };
    'main: loop {
        for event in event_loop.next(&mut events_loop) {
            if dull_event_routine(&display, &mut event_loop, &mut ui, &event) {
                break 'main;
            }
            render(&mut ui, &mut ids, &mut state);
            do_render(&display, &mut renderer, &image_map, &mut ui)
        }
    }
}

fn do_render(display: &GliumDisplayWinitWrapper, renderer: &mut Renderer, image_map: &conrod_core::image::Map<glium::texture::Texture2d>, ui: &mut Ui) -> () {
    if let Some(primitives) = ui.draw_if_changed() {
        renderer.fill(&display.0, primitives, &image_map);
        let mut target = display.0.draw();
        target.clear_color(0.0, 0.0, 0.0, 1.0);
        renderer.draw(&display.0, &mut target, &image_map).unwrap();
        target.finish().unwrap();
    }
}

fn init_font(ui: &mut Ui) -> (conrod_core::text::font::Id, conrod_core::text::font::Id) {
    let assets = find_folder::Search::KidsThenParents(3, 5)
        .for_folder("assets")
        .expect("assets folder not found");
    let font_path_light = assets.join("fonts/SourceHanSans/SourceHanSans-Light.ttf");
    let font_path_bold = assets.join("fonts/SourceHanSans/SourceHanSans-Bold.ttf");
    let bold = ui.fonts.insert_from_file(font_path_bold).expect("bold font file not found");
    let light = ui.fonts.insert_from_file(font_path_light).expect("light font file not found");
    return (light, bold);
}

fn dull_event_routine(display: &GliumDisplayWinitWrapper, event_loop: &mut EventLoop, ui: &mut Ui, event: &glium::glutin::Event) -> bool {
    if let Some(event) = support::convert_event(event.clone(), display) {
        ui.handle_event(event);
        event_loop.needs_update();
    }
    match event {
        glium::glutin::Event::WindowEvent { event, .. } => match event {
            glium::glutin::WindowEvent::CloseRequested
            | glium::glutin::WindowEvent::KeyboardInput {
                input:
                glium::glutin::KeyboardInput {
                    virtual_keycode: Some(glium::glutin::VirtualKeyCode::Escape),
                    ..
                },
                ..
            } => true,
            _ => false,
        },
        _ => false,
    }
}

fn render(ui: &mut Ui, ids: &mut Ids, state: &mut Context) {
    let ui = &mut ui.set_widgets();
    widget::Canvas::new().set(ids.canvas, ui);
    render_url_input_area(ui, ids, state);
    render_html_content(ui, ids, state);
}

fn render_url_input_area(ui: &mut UiCell, ids: &mut Ids, state: &mut Context) {
    widget::Canvas::new()
        .border(0.0)
        .mid_top_of(ids.canvas)
        .w_h(800.0, 50.0)
        .color(conrod_core::color::LIGHT_GRAY)
        .set(ids.url_input_area, ui);
    for input in widget::TextBox::new(state.current_url_input.as_str())
        .border(0.0)
        .w(750.0)
        .mid_left_with_margin_on(ids.url_input_area, 10.0)
        .set(ids.url_input, ui) {
        match input {
            Update(x) => state.current_url_input = x,
            _ => (),
        };
    }
    let go_button = widget::Button::new()
        .w(50.0)
        .border(0.0)
        .label("冲!")
        .color(conrod_core::color::GRAY)
        .mid_right_with_margin_on(ids.url_input_area, 10.0)
        .set(ids.visit_button, ui);
    for _event in go_button {
        refresh_page(state);
        ids.elements.resize(0, &mut ui.widget_id_generator())
    }
}

fn render_tag(ui: &mut UiCell,
              ids: &mut Ids,
              node: &Handle,
              state: &mut Context,
              current_selector: &mut Vec<String>,
              current_id_index: &mut usize,
              urls: &mut Vec<String>) {
    let children = node.children.borrow();
    match node.data {
        NodeData::Text { ref contents } => {
            let content = &contents.borrow();
            if !content.trim().is_empty() {
                ids.elements.resize(ids.elements.len() + 1, &mut ui.widget_id_generator());
                let font_id = if current_selector.iter().find(|it| it.starts_with('h') && it[1..].parse::<u8>().is_ok()).is_some() {
                    state.bold_font
                } else {
                    state.light_font
                };
                let font_size = if let Some(header) = current_selector.iter().find(|it| it.starts_with('h') && it[1..].parse::<u8>().is_ok()) {
                    let header_id = header[1..].parse::<u8>().unwrap();
                    *HEADER_SIZE.get(&header_id).unwrap()
                } else {
                    30.0
                };
                let href = urls.last();
                if current_selector.iter().find(|it| it.as_str() == "a").is_some() {
                    let mut the_widget = widget::Button::new()
                        .label(content)
                        .color(conrod_core::color::WHITE)
                        .label_color(conrod_core::color::BLUE)
                        .label_font_id(font_id)
                        .w_h(10.0 * content.len() as f64, 30.0)
                        .border(0.0);
                    if *current_id_index == 0usize {
                        the_widget = the_widget.top_left_of(ids.html_area);
                    }
                    for _click in the_widget.set(ids.elements[*current_id_index], ui) {
                        let url = href.unwrap().to_owned();
                        state.current_url_input = url;
                        refresh_page(state);
                        ids.elements.resize(0, &mut ui.widget_id_generator());
                        ids.elements.resize(1000, &mut ui.widget_id_generator());
                    }
                } else {
                    let mut the_widget = widget::Text::new(content)
                        .w_h(font_size / 2.0 * content.len() as f64, font_size)
                        .font_size(font_size as u32)
                        .font_id(font_id);
                    if *current_id_index == 0usize {
                        the_widget = the_widget.top_left_of(ids.html_area);
                    }
                    the_widget.set(ids.elements[*current_id_index], ui);
                }
                *current_id_index += 1;
            }
        }
        NodeData::Element { ref name, ref attrs, .. } => {
            let tag_name = format!("{}", name.local);
            if tag_name == "a" {
                let href = format!("{}", attrs.borrow().first().map(|it| &it.value).unwrap());
                urls.push(href);
            }
            assert!(name.ns == ns!(html));
            current_selector.push(tag_name);
            for child in children.iter() {
                render_tag(ui, ids, child, state, current_selector, current_id_index, urls);
            }
            current_selector.pop();
        }
        NodeData::Document { .. } => {
            for child in children.iter() {
                render_tag(ui, ids, child, state, current_selector, current_id_index, urls);
            }
        }
        _ => ()
    }
}

fn refresh_page(state: &mut Context) {
    let current_uri = &state.current_url_input["http://".len()..];
    let uri = current_uri.splitn(2, '/').collect::<Vec<_>>();
    let (host, sub) = (uri[0], uri[1]);
    let mut content = String::new();
    let mut stream = net::TcpStream::connect(format!("{}", host)).unwrap();
    stream.write(("GET ".to_string() + "/" + sub + " HTTP/1.1\r\nConnection: close\r\n\r\n").as_bytes())
        .expect("send request failed");
    stream.read_to_string(&mut content).expect("read response failed");
    stream.shutdown(Shutdown::Both).unwrap_or(());
    let parsed = parse(content.as_str());
    if parsed.status_code == "301" {
        state.current_url_input = parsed.headers.get("Location").unwrap().clone();
        refresh_page(state);
    } else if parsed.headers.get("Content-Type") == Some(&"text/html".to_string()) {
        state.dom_string = parsed.body;
    }
}

fn count_tags(node: &Handle) -> usize {
    match node.data {
        NodeData::Text { ref contents } => {
            let content = &contents.borrow();
            if !content.trim().is_empty() { 1 } else { 0 }
        }
        NodeData::Element { .. } | NodeData::Document { .. } => {
            node.children.borrow().iter()
                .map(|x| {
                    count_tags(x)
                })
                .fold(0, |acc, x| acc + x)
        }
        _ => 0
    }
}

fn render_html_content(ui: &mut UiCell, ids: &mut Ids, state: &mut Context) {
    widget::Canvas::new()
        .border(0.0)
        .w_h(800.0, 549.0)
        .color(conrod_core::color::WHITE)
        .mid_bottom_of(ids.canvas)
        .set(ids.html_area, ui);
    let dom = parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .one(state.dom_string.as_bytes());
    ids.elements.resize(1000, &mut ui.widget_id_generator());
    let mut selector = vec![];
    let mut urls = vec![];
    let mut current_id_index = 0;
    render_tag(ui, ids, &dom.document, state, &mut selector, &mut current_id_index, &mut urls);
}