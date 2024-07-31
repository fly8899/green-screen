use std::{
    collections::HashMap,
    io::Write,
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};

use itertools::Itertools;
use kamera::Camera;

fn main() {
    let state = Arc::new(Mutex::new(State::default()));
    let _ = start_tcp_listener(state.clone());
    start_camera(state);
}

struct State {
    filter: FilterType,
    con_count: usize,
    streams: HashMap<usize, TcpStream>,
}

impl State {
    fn process_payload(&mut self, payload: String) {
        self.streams
            .iter_mut()
            .filter_map(|(key, stream)| {
                let w = writeln!(stream, "{}", payload);
                let r = stream.flush();

                return if r.is_err() || w.is_err() {
                    Some(key.clone())
                } else {
                    None
                };
            })
            .collect::<Vec<usize>>()
            .into_iter()
            .for_each(|key| {
                self.streams.remove_entry(&key);
            });
    }

    fn insert_stream(&mut self, stream: TcpStream) {
        self.con_count += 1;
        self.streams.insert(self.con_count, stream);
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            filter: FilterType::Red,
            con_count: Default::default(),
            streams: Default::default(),
        }
    }
}

fn start_tcp_listener(state: Arc<Mutex<State>>) -> JoinHandle<()> {
    let handle = thread::spawn(move || {
        let listener = TcpListener::bind("127.0.0.1:8080").unwrap();
        println!("Listening at 127.0.0.1:8080");

        loop {
            for stream in listener.incoming() {
                if stream.is_err() {
                    continue;
                }

                let stream = stream.unwrap();
                let mut state = state.lock().unwrap();
                state.insert_stream(stream);
            }
        }
    });

    return handle;
}

fn start_camera(state: Arc<Mutex<State>>) {
    let camera = Camera::new_default_device();
    camera.start();

    let Some(frame) = camera.wait_for_frame() else {
        return;
    };

    let (w, h) = frame.size_u32();
    let back_ground_frame = frame.data().data_u8().to_colors();

    loop {
        let Some(frame) = camera.wait_for_frame() else {
            return;
        };

        let mut state = state.lock().unwrap();
        let current_frame = frame.data().data_u8().to_colors();
        let modified_frame = state.filter.apply_to(&back_ground_frame, current_frame);
        let payload = build_tcp_payload(w, h, modified_frame.to_buffer());
        state.process_payload(payload);
    }
}

fn build_tcp_payload(w: u32, h: u32, frame_buffer: Vec<u8>) -> String {
    let mut json = String::new();
    json.push('{');
    json.push_str(format!("\"width\": {w},").as_str());
    json.push_str(format!("\"height\": {h},").as_str());
    json.push_str("\"encoding-order\": \"RGBA\",");
    json.push_str(format!("\"image\": {:?}", frame_buffer).as_str());
    json.push('}');

    return json;
}

#[derive(Clone)]
struct Color {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl From<(&u8, &u8, &u8, &u8)> for Color {
    fn from(value: (&u8, &u8, &u8, &u8)) -> Self {
        let b = value.0.to_owned();
        let g = value.1.to_owned();
        let r = value.2.to_owned();
        let a = value.3.to_owned();

        return Color { r, g, b, a };
    }
}

impl From<Color> for [u8; 4] {
    fn from(value: Color) -> Self {
        return [value.r, value.g, value.b, value.a];
    }
}

trait ColorsToBuffer {
    fn to_buffer(&self) -> Vec<u8>;
}

impl ColorsToBuffer for [Color] {
    fn to_buffer(&self) -> Vec<u8> {
        return self
            .into_iter()
            .map(|color| Into::<[u8; 4]>::into(color.clone()))
            .flatten()
            .collect::<Vec<u8>>();
    }
}

trait BufferToColor {
    fn to_colors(&self) -> Vec<Color>;
}

impl BufferToColor for [u8] {
    fn to_colors(&self) -> Vec<Color> {
        return self
            .into_iter()
            .tuples::<(&u8, &u8, &u8, &u8)>()
            .map(|raw| raw.into())
            .collect::<Vec<Color>>();
    }
}

enum FilterType {
    Red,
    Blue,
    Green,
}

impl FilterType {
    fn apply_to(&self, back_ground_frame: &Vec<Color>, current_frame: Vec<Color>) -> Vec<Color> {
        return current_frame
            .into_iter()
            .enumerate()
            .map(|(index, color)| {
                return if self.should_cut_off(&color) {
                    back_ground_frame[index].clone()
                } else {
                    color
                };
            })
            .collect::<Vec<Color>>();
    }

    fn should_cut_off(&self, color: &Color) -> bool {
        let cut_off_range = 150..255;
        let cut_off_1 = 20;
        let cut_off_2 = 20;
        let cut_off_variance = 120;

        let target_color: u8;
        let mut target_color_variance: u32 = 0;

        match self {
            FilterType::Red => {
                target_color_variance += u32::from(color.g).abs_diff(cut_off_1);
                target_color_variance += u32::from(color.b).abs_diff(cut_off_2);
                target_color = color.r;
            }
            FilterType::Blue => {
                target_color_variance += u32::from(color.g).abs_diff(cut_off_1);
                target_color_variance += u32::from(color.r).abs_diff(cut_off_2);
                target_color = color.b;
            }
            FilterType::Green => {
                target_color_variance += u32::from(color.b).abs_diff(cut_off_1);
                target_color_variance += u32::from(color.r).abs_diff(cut_off_2);
                target_color = color.g;
            }
        };

        return cut_off_range.contains(&target_color) && target_color_variance < cut_off_variance;
    }
}

#[cfg(test)]
mod test {
    use crate::{BufferToColor, Color, ColorsToBuffer, FilterType};

    #[test]
    fn test_process_frame_buffer_len() {
        let buffer: Vec<u8> = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let result = &buffer.to_colors();

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_fix_buffer_order() {
        let buffer1: Vec<u8> = vec![1, 2, 3, 4];
        let buffer2 = buffer1.to_colors().to_buffer();

        let ordered: Vec<u8> = vec![3, 2, 1, 4];
        assert_eq!(buffer2, ordered)
    }

    #[test]
    fn test_filter() {
        let target1 = Color {
            r: 235,
            g: 20,
            b: 10,
            a: 5,
        };

        let target2 = Color {
            r: 100,
            g: 50,
            b: 10,
            a: 5,
        };

        let filter = FilterType::Red;
        assert_eq!(filter.should_cut_off(&target1), true);
        assert_eq!(filter.should_cut_off(&target2), false);
    }
}
