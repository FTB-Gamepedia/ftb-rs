// Copyright © 2014, Peter Atashian

use serialize::json::decode;
use std::collections::HashMap;
use std::collections::hashmap::{
    Occupied,
    Vacant,
};
use std::io::{
    File,
    stdin,
};
use std::io::stdio::flush;

#[deriving(Decodable)]
struct Recipe {
    input: HashMap<String, uint>,
    quantity: uint,
}
#[deriving(Decodable)]
struct Recipes {
    recipes: HashMap<String, Recipe>,
    existing: HashMap<String, uint>,
}
#[deriving(Show)]
struct RecipeCalc {
    existing: HashMap<String, uint>,
    tocraft: HashMap<String, uint>,
    needed: HashMap<String, uint>,
    used: HashMap<String, uint>,
}
impl Recipes {
    fn new() -> Recipes {
        let mut file = File::open(&Path::new("work/recipes.js")).unwrap();
        let data = file.read_to_string().unwrap();
        decode(data.as_slice()).unwrap()
    }
    fn lookup(&self, name: &str, quantity: uint) {
        let name = name.into_string();
        let mut calc = RecipeCalc {
            existing: self.existing.clone(),
            tocraft: HashMap::new(),
            needed: HashMap::new(),
            used: HashMap::new(),
        };
        let mut out = File::create(&Path::new("work/todo.txt")).unwrap();
        (writeln!(out, "Crafting {} of {}", quantity, name)).unwrap();
        self.find(&name, &mut calc, quantity);
        (writeln!(out, "Existing items used {{")).unwrap();
        for (item, &count) in calc.existing.iter() {
            let ocount = *self.existing.find(item).unwrap_or(&0);
            if ocount > count { (writeln!(out, "   {}: {}", item, ocount - count)).unwrap() }
        }
        (writeln!(out, "}}")).unwrap();
        (writeln!(out, "Crafted leftovers {{")).unwrap();
        for (item, &count) in calc.existing.iter() {
            let ocount = *self.existing.find(item).unwrap_or(&0);
            if count > ocount { (writeln!(out, "   {}: {}", item, count - ocount)).unwrap() }
        }
        (writeln!(out, "}}")).unwrap();
        (writeln!(out, "Things still needed {{")).unwrap();
        for (item, &count) in calc.needed.iter() {
            (writeln!(out, "   {}: {}", item, count)).unwrap();
        }
        (writeln!(out, "}}")).unwrap();
        (writeln!(out, "Things to craft {{")).unwrap();
        for (item, &count) in calc.tocraft.iter() {
            let recipe = self.recipes.find(item).unwrap();
            (writeln!(out, "   {}: {} {{", item, count * recipe.quantity)).unwrap();
            for (thing, &amount) in recipe.input.iter() {
                (writeln!(out, "      {}: {}", thing, count * amount)).unwrap();
            }
            (writeln!(out, "   }}")).unwrap();
        }
        (writeln!(out, "}}")).unwrap();

    }
    fn find(&self, name: &String, out: &mut RecipeCalc, quantity: uint) {
        let quantity = match out.existing.find_mut(name) {
            Some(num) => {
                if *num >= quantity {
                    match out.used.entry(name.clone()) {
                        Occupied(x) => *x.into_mut() += quantity,
                        Vacant(x) => { x.set(quantity); },
                    }
                    *num -= quantity; return
                } else {
                    match out.used.entry(name.clone()) {
                        Occupied(x) => *x.into_mut() += *num,
                        Vacant(x) => { x.set(*num); },
                    }
                    let rem = quantity - *num; *num = 0; rem
                }
            },
            None => quantity,
        };
        match self.recipes.find(name) {
            Some(recipe) => {
                let num = (quantity as f32 / recipe.quantity as f32).ceil() as uint;
                match out.existing.entry(name.clone()) {
                    Occupied(x) => *x.into_mut() += num * recipe.quantity - quantity,
                    Vacant(x) => { x.set(num * recipe.quantity - quantity); },
                }
                match out.tocraft.entry(name.clone()) {
                    Occupied(x) => *x.into_mut() += num,
                    Vacant(x) => { x.set(num); },
                }
                for (item, &amount) in recipe.input.iter() {
                    self.find(item, out, num * amount);
                }
            },
            None => {
                match out.needed.entry(name.clone()) {
                    Occupied(x) => *x.into_mut() += quantity,
                    Vacant(x) => { x.set(quantity); },
                }
            }
        }
    }
}
pub fn do_recipe_calc() {
    let rec = Recipes::new();
    let mut cin = stdin();
    print!("Desired output: ");
    flush();
    let thing = cin.read_line().unwrap();
    let thing = thing.as_slice();
    let thing = thing.slice_to(thing.find(['\r', '\n'].as_slice()).unwrap_or(thing.len()));
    print!("Desired quantity: ");
    flush();
    let quantity = cin.read_line().unwrap();
    let quantity = quantity.as_slice();
    let quantity = quantity.slice_to(quantity.find(['\r', '\n'].as_slice()).unwrap_or(quantity.len()));
    let quantity = from_str(quantity).unwrap();
    rec.lookup(thing, quantity);
}
