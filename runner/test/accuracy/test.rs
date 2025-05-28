use std::io::{self, BufRead};

fn main() {
    let stdin = io::stdin();

    for line in stdin.lock().lines() {
        if let Ok(text) = line {
            let parts: Vec<&str> = text.trim().split_whitespace().collect();
            if parts.len() == 2 {
                if let (Ok(a), Ok(b)) = (parts[0].parse::<i32>(), parts[1].parse::<i32>()) {
                    println!("{}", a + b);
                }
            }
        }
    }
}