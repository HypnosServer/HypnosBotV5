use std::collections::{HashMap, HashSet, VecDeque};

use valence_anvil::RegionFolder;
use valence_nbt::{Compound, Value};

struct Map<'a> {
    region: &'a mut RegionFolder,
    cached_chunks: HashMap<(i32, i32), Compound>,
}

impl<'a> Map<'a> {
    fn new(region: &'a mut RegionFolder) -> Self {
        Self {
            region: region,
            cached_chunks: HashMap::new(),
        }
    }

    fn get_chunk(&mut self, x: i32, z: i32) -> Option<Compound> {
        if let Some(chunk) = self.cached_chunks.get(&(x, z)) {
            return Some(chunk.clone());
        }
        for _ in 0..3 {
            if let Ok(Some(chunk)) = self.region.get_chunk(x, z) {
                self.cached_chunks.insert((x, z), chunk.data.clone());
                return Some(chunk.data);
            }
        }
        None
    }
}

fn get_block_ic(x: i64, y: i64, z: i64, chunk: &Compound) -> Option<(u8, u8)> {
    let Some(Value::Compound(chunk_data)) = chunk.get("Level") else {
        return None;
    };
    let Some(Value::List(subchunks)) = chunk_data.get("Sections") else {
        return None;
    };
    let subchunk = y / 16;
    let subchunk_index = subchunk as usize;
    if subchunk_index >= subchunks.len() {
        return None;
    }
    let subchunk = subchunks.get(subchunk_index)?;
    let Value::Compound(subchunk_data) = subchunk.to_value() else {
        return None;
    };
    let x_bounded = x.rem_euclid(16);
    let y_bounded = y.rem_euclid(16);
    let z_bounded = z.rem_euclid(16);
    let block_index = (y_bounded * 16 + z_bounded) * 16 + x_bounded;
    let Some(Value::ByteArray(blocks)) = subchunk_data.get("Blocks") else {
        return None;
    };

    let Some(Value::ByteArray(data)) = subchunk_data.get("Data") else {
        return None;
    };

    let block_id = blocks.get(block_index as usize)?;
    let rest = block_index % 2;
    // Data is packed in 4-bit nibbles, so we need to extract the correct nibble
    let data_index = block_index / 2;
    let data_value = if rest == 0 {
        data.get(data_index as usize)? & 0x0F
    } else {
        data.get(data_index as usize)? >> 4 & 0x0F
    };
    Some((*block_id as u8, data_value as u8))
}

fn get_block(x: i64, y: i64, z: i64, region: &mut Map) -> u8 {
    let chunk = region.get_chunk(x.div_euclid(16) as i32, z.div_euclid(16) as i32);
    if let Some(chunk) = chunk {
        if let Some((block_id, data)) = get_block_ic(x, y, z, &chunk) {
            return block_id;
        }
    }
    return 0;
}

fn find_neighbors(x: i64, y: i64, z: i64, region: &mut Map, block_id: u8) -> [bool; 8] {
    let mut neighbors = [false; 8];
    // Iterate y level up and down from closest to farthest from y, the entire y range is 0-255
    'outer: for i in 0..=255 {
        let y_up = y + i;
        let y_down = y - i;
        if y_up > 255 && y_down < 0 {
            break;
        }
        for i in 0..8 {
            let (dx, dz) = match i {
                0 => (1, 0),   // +x
                1 => (-1, 0),  // -x
                2 => (0, 1),   // +z
                3 => (0, -1),  // -z
                4 => (1, 1),   // +x +z
                5 => (1, -1),  // +x -z
                6 => (-1, 1),  // -x +z
                7 => (-1, -1), // -x -z
                _ => (0, 0),
            };
            if y_up <= 255 && get_block(x + dx, y_up, z + dz, region) == block_id {
                neighbors[i] = true;
            }
            if y_down >= 0 && get_block(x + dx, y_down, z + dz, region) == block_id {
                neighbors[i] = true;
            }
        }
    }
    neighbors
}

struct Grid {
    min_x: i64,
    max_x: i64,
    min_z: i64,
    max_z: i64,
    data: Vec<bool>,
}

impl Grid {
    fn new(min_x: i64, max_x: i64, min_z: i64, max_z: i64) -> Self {
        let width = (max_x - min_x + 1) as usize;
        let height = (max_z - min_z + 1) as usize;
        Self {
            min_x,
            max_x,
            min_z,
            max_z,
            data: vec![false; width * height],
        }
    }

    fn flood_fill(&mut self, start_x: i64, start_z: i64) {
        let mut expanded = expand_by_one(
            &self.data.clone(),
            (self.max_x - self.min_x + 1) as usize,
            (self.max_z - self.min_z + 1) as usize,
        );
        flood_fill_outside(
            &mut expanded,
            (self.max_x - self.min_x + 3) as usize,
            (self.max_z - self.min_z + 3) as usize,
            0,
            0,
        );
        for z in self.min_z..=self.max_z {
            for x in self.min_x..=self.max_x {
                let expanded_index = ((z - self.min_z + 1) * (self.max_x - self.min_x + 3)
                    + (x - self.min_x + 1)) as usize;
                let index =
                    ((z - self.min_z) * (self.max_x - self.min_x + 1) + (x - self.min_x)) as usize;
                self.data[index] = !expanded[expanded_index];
            }
        }
    }
}

fn expand_by_one(map: &Vec<bool>, width: usize, height: usize) -> Vec<bool> {
    let mut expanded = vec![false; (width + 2) * (height + 2)];
    for y in 0..height {
        for x in 0..width {
            let index = y * width + x;
            if map[index] {
                let expanded_index = (y + 1) * (width + 2) + (x + 1);
                expanded[expanded_index] = true;
            }
        }
    }
    expanded
}

fn flood_fill_outside(
    map: &mut Vec<bool>,
    width: usize,
    height: usize,
    start_x: usize,
    start_y: usize,
) {
    let mut queue = VecDeque::new();
    queue.push_back((start_x, start_y));
    while let Some((x, y)) = queue.pop_front() {
        let index = y * width + x;
        if map[index] {
            continue;
        }
        map[index] = true;
        if x > 0 {
            queue.push_back((x - 1, y));
        }
        if x < width - 1 {
            queue.push_back((x + 1, y));
        }
        if y > 0 {
            queue.push_back((x, y - 1));
        }
        if y < height - 1 {
            queue.push_back((x, y + 1));
        }
    }
}

fn count_air(x: i64, y_range: (i64, i64), z: i64, chunk: &Compound) -> u32 {
    let mut count = 0;
    for y in y_range.0..=y_range.1 {
        if let Some((block_id, data)) = get_block_ic(x, y, z, chunk) {
            if block_id == 0 {
                count += 1;
            }
        }
    }
    count
}

pub fn perimeter_count(region_folder: &mut RegionFolder, start_pos: (i64, i64, i64)) -> (u32, u32) {
    let mut map = Map::new(region_folder);
    let mut shape = HashSet::new();
    let block_id = get_block(start_pos.0, start_pos.1, start_pos.2, &mut map);
    let mut to_visit = vec![start_pos];
    while let Some((x, y, z)) = to_visit.pop() {
        if shape.contains(&(x, y, z)) {
            continue;
        }
        shape.insert((x, y, z));
        let neighbors = find_neighbors(x, y, z, &mut map, block_id);
        for i in 0..8 {
            if neighbors[i] {
                let (dx, dz) = match i {
                    0 => (1, 0),   // +x
                    1 => (-1, 0),  // -x
                    2 => (0, 1),   // +z
                    3 => (0, -1),  // -z
                    4 => (1, 1),   // +x +z
                    5 => (1, -1),  // +x -z
                    6 => (-1, 1),  // -x +z
                    7 => (-1, -1), // -x -z
                    _ => (0, 0),
                };
                to_visit.push((x + dx, y, z + dz));
            }
        }
    }
    let vec = shape.clone().into_iter().collect::<Vec<_>>();
    let min_x = vec.iter().map(|(x, _, _)| *x).min().unwrap();
    let max_x = vec.iter().map(|(x, _, _)| *x).max().unwrap();
    let min_z = vec.iter().map(|(_, _, z)| *z).min().unwrap();
    let max_z = vec.iter().map(|(_, _, z)| *z).max().unwrap();
    let mut flood_shape = Vec::new();
    for z in min_z..=max_z {
        for x in min_x..=max_x {
            if shape.contains(&(x, start_pos.1, z)) {
                flood_shape.push(true);
            } else {
                flood_shape.push(false);
            }
        }
    }
    let mut grid = Grid::new(min_x, max_x, min_z, max_z);
    grid.data = flood_shape;
    grid.flood_fill(start_pos.0, start_pos.2);
    let true_count = grid.data.iter().filter(|b| **b).count();
    println!("True count: {}", true_count);
    let mut block_count = 0;
    let mut air_count = 0;
    let y_range = (5, 63);
    // Map with c
    let min_z_chunk = min_z.div_euclid(16);
    let max_z_chunk = max_z.div_euclid(16);
    let min_x_chunk = min_x.div_euclid(16);
    let max_x_chunk = max_x.div_euclid(16);
    for z_chunk in min_z_chunk..=max_z_chunk {
        for x_chunk in min_x_chunk..=max_x_chunk {
            let chunk = map.get_chunk(x_chunk as i32, z_chunk as i32);
            if let Some(chunk) = chunk {
                for z in (z_chunk * 16)..((z_chunk + 1) * 16) {
                    for x in (x_chunk * 16)..((x_chunk + 1) * 16) {
                        if z < min_z || z > max_z || x < min_x || x > max_x {
                            continue;
                        }
                        let index = ((z - min_z) * (max_x - min_x + 1) + (x - min_x)) as usize;
                        if grid.data[index] {
                            block_count += (y_range.1 - y_range.0 + 1) as u32;
                            air_count += count_air(x, y_range, z, &chunk);
                        }
                    }
                }
            }
        }
    }
    //for z in min_z..=max_z {
    //    for x in min_x..=max_x {
    //        if grid.data[((z - min_z) * (max_x - min_x + 1) + (x - min_x)) as usize] {
    //            block_count += (y_range.1 - y_range.0 + 1) as u32;
    //            air_count += count_air(x, y_range, z, &mut map);
    //        }
    //    }
    //}
    println!("Block count: {}, Air count: {}", block_count, air_count);
    return (block_count, air_count);
}
