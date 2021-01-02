use wasm_bindgen::{prelude::*, Clamped};
use lazy_static::lazy_static;
use web_sys;
use wasm_bindgen::JsCast;
use std::f64::consts::PI;
use web_sys::{ImageData};
use std::sync::{MutexGuard, Mutex};
use std::vec;

lazy_static! {
    static ref ARRAY: Mutex<Vec<f64>> = Mutex::new(vec![]);
    static ref RANDOMS: Mutex<Vec<f64>> = Mutex::new(vec![]);
}

struct RadarSettings {
    beamwidth: f64,
    rain_interf: bool,
    radar_interf: bool,
}

#[wasm_bindgen]
pub fn init_panic_hook() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
extern {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);

    #[wasm_bindgen(js_namespace = Math)]
    fn random() -> f64;
}

#[wasm_bindgen]
pub fn create_heightmap(w: u32, img_data: js_sys::Array) {
    let mut randoms = RANDOMS.lock().unwrap();
    for _ in 0..w*w {
        randoms.push(random());
    }
    drop(randoms);
    let mut heightmap = ARRAY.lock().unwrap();
    heightmap.clear();
    for x in (0..w*w*4).step_by(4) {
        let r = img_data.get(x).as_f64().unwrap();
        let g = img_data.get(x+1).as_f64().unwrap();
        let b = img_data.get(x+2).as_f64().unwrap();
        heightmap.push(match pixeldata_to_height(r, g, b){
            p if p > 8.0 => p * 10.0,
            _ => 0.0
        });
    }
    drop(heightmap);
}

#[wasm_bindgen]
pub fn draw_into_canvas(bposx: f64, bposy: f64, beamwidth: f64, rain_interf: bool, radar_interf: bool) {
    let mut image:Vec<u8> = Vec::new();
    for x in 0..512*512*4 {
        image.push(match x % 4 {
            1 => 250,
            _ => 0
        })
    }

    let settings = RadarSettings {
        beamwidth: beamwidth,
        radar_interf: match beamwidth {
            p if p > 2.0 => radar_interf,
            _ => false
        },
        rain_interf: rain_interf
    };

    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let canvas = document
    .get_element_by_id("canvas")
    .unwrap()
    .unchecked_into::<web_sys::HtmlCanvasElement>();
    
    let context = canvas
    .get_context("2d")
    .unwrap()
    .unwrap()
    .unchecked_into::<web_sys::CanvasRenderingContext2d>();
    
    process_image(512, &mut image, bposx, bposy, settings);
    let data = ImageData::new_with_u8_clamped_array_and_sh(Clamped(&mut image), 512, 512)
    .unwrap();
    
    match context.put_image_data(&data, 0.0, 0.0) {
        Ok(_) => (),
        _ => panic!("There was an error putting data into canvas")
    }
}

fn draw_ray_by_norm_dir(dirx: f64, diry: f64, bpx: f64, bpy: f64, settings: &RadarSettings, img_width: u32, image: &mut Vec<u8>, heightmap: &MutexGuard<'_, Vec<f64>>) {
    let mut next_pos = (bpx, bpy);
    let mut has_hit = false;
    let mut min = 10;

    while next_pos.0 > 0.0 && next_pos.1 > 0.0 && next_pos.0 < img_width as f64 && next_pos.1 < img_width as f64 {
        let height = match pixel_data_at(next_pos.0, next_pos.1, img_width, &heightmap, false) {
            Some(val) => val,
            None => return () 
        };

        min = match height {
            p if p > min => height,
            _ => min
        };

        if settings.radar_interf && random() > 0.5 {
            image[index_from_pos(next_pos.0 as u32, next_pos.1 as u32, img_width, true).unwrap() + 3 as usize] = 220;
        }
        
        if height == min && !has_hit {
            let bpdeltax = next_pos.0 - bpx;
            let bpdeltay = next_pos.1 - bpy;
            let distance_from_origin = (bpdeltax*bpdeltax + bpdeltay*bpdeltay).sqrt();
            
            //Approximation of errors due to beamwidth
            let error_length = (distance_from_origin * settings.beamwidth.to_radians()).ceil() as u32;
            let mat = [0, -1, 1, 0];
            let error_dir = (
                dirx * mat[0] as f64 + diry * mat[2] as f64,
                dirx * mat[1] as f64 + diry * mat[3] as f64
            );

            let mut next_err_location = (next_pos.0, next_pos.1);
            for _ in 0..error_length {
                let index = index_from_pos(next_err_location.0.round() as u32, next_err_location.1.round() as u32, img_width, true);
                match index{
                    Some(num) => image[num + 3] = 255,
                    _ => {} 
                };
                next_err_location.0 += error_dir.0;
                next_err_location.1 += error_dir.1;
            }
            //approximation of the radar "seeing over" things.
        } else if height < min - 50 && min > 10 {
            has_hit = true;
        }
        next_pos = (next_pos.0 + dirx, next_pos.1 + diry);
    }
}

fn process_image(img_width: u32, image: &mut Vec<u8>, bposx: f64, bposy: f64, settings: RadarSettings) {
    let mut angle = 0.0;
    let beamwidth_rad = settings.beamwidth.to_radians();
    let heightmap = ARRAY.lock().unwrap();
    while angle < PI * 2.0 {
        draw_ray_by_norm_dir(angle.sin(), angle.cos(), bposx, bposy, &settings,  img_width, image, &heightmap);
        angle += beamwidth_rad;
    }
    if settings.rain_interf {
        rainify(bposx as u32, bposy as u32, img_width, image);
    }
}

fn rainify(bposx: u32, bposy: u32, img_width: u32, image: &mut Vec<u8>) {
    //the random libraries supporting WASM targets where very slow.
    //calling the javascript Math.random was also slow
    //So randoms are generated once per map, then reused
    let randoms = RANDOMS.lock().unwrap();
    for y in 0..img_width {
        let random_start = (random() * (img_width*img_width) as f64) as u32 ; 
        for x in 0..img_width {
            let bpdeltax = x - bposx;
            let bpdeltay = y - bposy;
            let distance_from_origin_squared = bpdeltax * bpdeltax + bpdeltay * bpdeltay;
            let some_num = 70;
            let random = randoms[((random_start + x) % (img_width*img_width)) as usize]; 
            if random > 0.95 || ((random - 0.5) * distance_from_origin_squared as f64 ) as u32 > some_num * some_num {
                image[index_from_pos(x, y, img_width, true).unwrap() + 3] = 128;
            }
        }
    }
    drop(randoms);
}

fn index_from_pos(x: u32, y: u32, width: u32, rgba: bool) -> Option<usize> {
    if x >= width || y >= width {
        return None;
    }
    match rgba {
        true => Some(((x + y * width) * 4) as usize),
        _ => Some((x + y * width) as usize)
    }
}

fn pixel_data_at(x: f64, y: f64, width: u32, heightmap: &MutexGuard<'_, Vec<f64>, >, rgba: bool) -> Option<u8> {
    match index_from_pos(x as u32, y as u32, width, rgba) {
        Some(val) => Some(heightmap[val] as u8),
        None => None 
    }
}

fn pixeldata_to_height(r: f64, g: f64, b: f64) -> f64 {
    (r * 256.0 + g + b / 256.0) - 32768.0
}