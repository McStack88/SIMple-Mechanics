use crate::main_state::{body_builder::BodyBuilder, MainState};
use crate::resources::LuaRes;

use crate::components::{Collider, Color, Name, PhysicsBody};
use crate::resources::{self, Paused, ShapeInfo};

use crate::{BodySet, ColliderSet, MechanicalWorld, RigidBody, Vector};
use np::material::BasicMaterial;
use np::object::Body;
use nphysics2d as np;

use nc::shape::{Ball, Cuboid};
use ncollide2d as nc;

// use resources::Paused;
use specs::prelude::*;

use rlua::prelude::*;

// TODO:
// A way to interact with shapes that have already been instantiated,
// also a way to set custom collision math
//
// This is more difficult than it seems since it's difficult to access
// the ECS fom a Lua function.
//
// The solution I'm thinking of is having the user pass in a Lua function
// which accepts a physics object as a Lua table and outputs a new Lua table
// physics object.
//
// Using this pure sort of function extra Lua glue can be written so that users
// can easily construct functions which handle integration and collision math separately.

pub trait LuaResExt {
    fn run_lua_code(&mut self, code: String);
    fn run_lua_file(&self, filename: impl AsRef<std::path::Path> + std::clone::Clone);
}

pub fn new_lua_res() -> LuaRes {
    let mut lua_stdlib = rlua::StdLib::empty();
    lua_stdlib.insert(rlua::StdLib::BASE);
    lua_stdlib.insert(rlua::StdLib::TABLE);
    lua_stdlib.insert(rlua::StdLib::MATH);
    lua_stdlib.insert(rlua::StdLib::STRING);

    let lua = Lua::new_with(lua_stdlib);
    lua.set_memory_limit(Some(262_144));
    lua.set_hook(
        rlua::HookTriggers {
            every_nth_instruction: Some(75_000),
            ..Default::default()
        },
        |_, _| panic!("Lua script exceeded instruction limit"),
    );

    lua.context(|lua_ctx| {
        let globals = lua_ctx.globals();
        let shapes: Vec<rlua::Table> = Vec::new();
        globals.set("shapes", shapes).unwrap();
        globals.set("PAUSED", false).unwrap();
        globals.set("GRAVITY", 9.81).unwrap();
        globals.set("PI", std::f32::consts::PI).unwrap();
        globals.set("SCREEN_X", crate::SCREEN_X).unwrap();
        globals.set("SCREEN_Y", crate::SCREEN_Y).unwrap();

        lua_ctx
            .load(
                r#"
                    function add_shape(shape)
                        shapes[#shapes+1] = shape
                    end

                    function add_shapes(...)
                        for _, shape in ipairs{...} do
                            add_shape(shape)
                        end
                    end

                    function update()
                    end
                "#,
            )
            .exec()
            .unwrap();
    });

    std::sync::Arc::new(std::sync::Mutex::new(lua))
}

impl LuaResExt for LuaRes {
    fn run_lua_code(&mut self, code: String) {
        self.lock().unwrap().context(|lua_ctx| {
            lua_ctx.load(&code).exec().unwrap();
        });
    }

    fn run_lua_file(&self, filename: impl AsRef<std::path::Path> + std::clone::Clone) {
        self.lock().unwrap().context(|lua_ctx| {
            let lua_code = std::fs::read_to_string(filename.clone()).unwrap();
            if let Err(e) = lua_ctx
                .load(&lua_code)
                .set_name(&filename.as_ref().file_name().unwrap().to_str().unwrap())
                .unwrap()
                .exec()
            {
                println!("Lua {}", e.to_string());
            };
        });
    }
}

impl<'a, 'b> MainState<'a, 'b> {
    #[allow(clippy::many_single_char_names)]
    /// must call world.maintain() after calling this for shape to actually get added
    /// in practice is only used in process_lua_shapes() so it should be fine
    pub fn process_lua_shape(&mut self, shape: &rlua::Table) {
        let ty: String = shape.get("shape").unwrap();
        let mass = shape.get("mass").unwrap_or(1.0);
        let x = shape.get("x").unwrap();
        let y = shape.get("y").unwrap();
        let x_vel = shape.get("x_vel").unwrap_or(0.0);
        let y_vel = shape.get("y_vel").unwrap_or(0.0);
        let rotvel = shape.get("rotvel").unwrap_or(0.0);
        let rotation = shape.get("rotation").unwrap_or(0.0);
        let elasticity = shape.get("elasticity").unwrap_or(0.2);
        let friction = shape.get("friction").unwrap_or(0.5);
        let name = shape.get("name");
        let status = shape
            .get("status")
            .unwrap_or_else(|_| "dynamic".to_string());
        let color = shape
            .get("color")
            .map_or(ggez::graphics::WHITE, |color: rlua::Table| {
                let r = color.get("r").unwrap();
                let g = color.get("g").unwrap();
                let b = color.get("b").unwrap();
                let a = color.get("a").unwrap_or(255);
                ggez::graphics::Color::from_rgba(r, g, b, a)
            });

        #[allow(clippy::wildcard_in_or_patterns)]
        let status = match status.to_lowercase().as_str() {
            "static" => np::object::BodyStatus::Static,
            "kinematic" => np::object::BodyStatus::Kinematic,
            "dynamic" | _ => np::object::BodyStatus::Dynamic,
        };

        let shape_info = match ty.to_lowercase().as_str() {
            "rectangle" | "rect" => {
                let w = shape.get("w").unwrap();
                let h = shape.get("h").unwrap();
                ShapeInfo::Rectangle(Some(Vector::new(w, h)))
            }
            "circle" => {
                let rad = shape.get("r").unwrap();
                ShapeInfo::Circle(Some(rad))
            }
            _ => panic!("invalid shape"),
        };

        BodyBuilder {
            translation: Vector::new(x, y),
            rotation,
            velocity: Vector::new(x_vel, y_vel),
            rotvel,
            status,
            restitution: elasticity,
            friction,
            color,
            name: name.ok(),
            ..BodyBuilder::from_world(&self.world, shape_info, mass)
        }
        .create();
    }

    pub fn process_lua_shapes(&mut self, shapes: Vec<rlua::Table>) {
        shapes
            .iter()
            .for_each(|shape| self.process_lua_shape(shape));
        self.world.maintain();
    }

    pub fn add_shapes_from_lua_file(
        &mut self,
        filename: impl AsRef<std::path::Path> + std::clone::Clone,
    ) {
        let lua = self.world.fetch_mut::<LuaRes>().clone();
        lua.run_lua_file(filename);
        lua.lock().unwrap().context(|lua_ctx| {
            let globals = lua_ctx.globals();
            let shapes = globals.get::<_, Vec<rlua::Table>>("shapes").unwrap();
            self.process_lua_shapes(shapes);

            let shapes: Vec<rlua::Table> = Vec::new();
            globals.set("shapes", shapes).unwrap();
        });
    }

    pub fn export_lua(&self, filename: impl AsRef<std::path::Path> + std::clone::Clone) {
        let mut body_string = String::with_capacity(48);

        let physics_bodies = self.world.read_storage::<PhysicsBody>();
        let body_set = self.world.fetch::<BodySet>();

        let colliders = self.world.read_storage::<Collider>();
        let collider_set = self.world.fetch::<ColliderSet>();

        let colors = self.world.read_storage::<Color>();

        let mut first = true;

        (&physics_bodies, &colliders, &colors).join().for_each(|(physics_body_handle, collider_handle, color)|{
            if !first {
                body_string.push_str(",\n\t");
            } else {
                first = false;
            }

            let rigid_body = body_set.get(physics_body_handle.body_handle).unwrap().downcast_ref::<RigidBody>().unwrap();
            let collider = collider_set.get(collider_handle.coll_handle).unwrap();

            let (shape_info_str, shape_str) = {
                let shape = collider.shape();
                if shape.is_shape::<Ball<f32>>() {
                    let ball = shape.downcast_ref::<Ball<f32>>().unwrap_or_else(|| unreachable!());
                    let shape_info_str = format!("r = {}", ball.radius());
                    (shape_info_str, "Circle")
                } else if shape.is_shape::<Cuboid<f32>>() {
                    let cuboid = shape.downcast_ref::<Cuboid<f32>>().unwrap_or_else(|| unreachable!());
                    let half_extents = cuboid.half_extents();
                    let shape_info_str = format!("w = {}, h = {}", half_extents.x, half_extents.y);
                    (shape_info_str, "Rect")
                } else {
                    panic!("Serialize invalid shape")
                }
            };

            let position = rigid_body.position();
            let velocity = rigid_body.velocity();

            let material = collider.material().downcast_ref::<BasicMaterial<f32>>().unwrap();

            let status_str = match rigid_body.status() {
                np::object::BodyStatus::Static => "static",
                np::object::BodyStatus::Dynamic => "dynamic",
                _ => panic!("Invalid body status for serialization"),
            };

            body_string.push_str(
                format!(
                    "{{shape = \"{shape_str}\", x = {x:.prec$}, y = {y:.prec$}, rotation = {rotation:.prec$}, x_vel = {x_vel:.prec$}, y_vel = {y_vel:.prec$}, rotvel = {rotvel:.prec$}, {shape_info_str}, mass = {mass:.prec$}, friction = {friction:.prec$}, elasticity = {elasticity:.prec$}, color = {{r = {red}, g = {green}, b = {blue}}}, status = \"{status}\"}}",
                    shape_str = shape_str,
                    x = position.translation.x,
                    y = position.translation.y,
                    rotation = position.rotation.angle(),
                    x_vel = velocity.linear.x,
                    y_vel = velocity.linear.y,
                    rotvel = velocity.angular,
                    shape_info_str = shape_info_str,
                    mass = rigid_body.augmented_mass().linear,
                    friction = material.friction,
                    elasticity = material.restitution,
                    status = status_str,
                    red = (color.0.r * 255.0).round() as usize,
                    green = (color.0.g * 255.0).round() as usize,
                    blue = (color.0.b * 255.0).round() as usize,
                    prec = 3,
                ).as_str())
        });

        let body_string = format!("add_shapes(\n\t{}\n)\n", body_string);

        let mut consts_str = String::with_capacity(48);
        let gravity = {
            let mech_world = self.world.fetch::<MechanicalWorld>();
            mech_world.gravity.y
        };
        consts_str.push_str(format!("GRAVITY = {}\n", gravity).as_str());

        let lua_string = format!("{}\n{}", body_string, consts_str);

        std::fs::write(filename, lua_string).unwrap();
    }

    pub fn lua_update(&mut self) {
        let lua = self.world.fetch_mut::<crate::resources::LuaRes>().clone();
        lua.lock().unwrap().context(|lua_ctx| {
            // doesn't work right now because it hits the instruction limit eventually
            // targeted for a later release
            // lua_ctx.load("update()").exec().unwrap();

            let globals = lua_ctx.globals();
            if let Ok(true) = globals.get("ADD_SHAPES") {
                self.process_lua_shapes(globals.get::<_, Vec<rlua::Table>>("shapes").unwrap());
            }

            if let Ok(paused) = globals.get::<_, bool>("PAUSED") {
                self.world.insert::<Paused>(Paused(paused));
            }
            if let Ok(gravity) = globals.get::<_, f32>("GRAVITY") {
                self.world.fetch_mut::<MechanicalWorld>().gravity.y = gravity;
            }

            globals.set("ADD_SHAPES", false).unwrap();
            globals
                .set("FPS", self.world.fetch::<resources::FPS>().0)
                .unwrap();
            globals
                .set("DT", self.world.fetch::<resources::DT>().0.as_millis())
                .unwrap();

            {
                let scale_fac = self.world.fetch::<resources::ScaleFac>().0;

                globals
                    .set("SCREEN_X", crate::SCREEN_X * scale_fac.x)
                    .unwrap();
                globals
                    .set("SCREEN_Y", crate::SCREEN_Y * scale_fac.y)
                    .unwrap();
            }

            {
                pub struct LuaEntity(pub Entity);
                impl rlua::UserData for LuaEntity {
                    fn add_methods<'lua, M: rlua::UserDataMethods<'lua, Self>>(methods: &mut M) {
                        methods.add_method("id", |_, this, _: ()| Ok(this.0.id()));
                    }
                }

                let entities = self.world.entities();
                let names = self.world.read_storage::<Name>();
                let lua_objects = lua_ctx.create_table().unwrap();
                (&entities, &names).join().for_each(|(entity, name)| {
                    lua_objects.set(name.0.as_str(), LuaEntity(entity)).unwrap();
                });
                globals.set("OBJECTS", lua_objects).unwrap();
            }

            {
                let mouse_pos = self.world.fetch::<resources::MousePos>().0;
                globals.set("MOUSE_X", mouse_pos.x).unwrap();
                globals.set("MOUSE_Y", mouse_pos.y).unwrap();
            }

            let shapes: Vec<rlua::Table> = Vec::new();
            globals.set("shapes", shapes).unwrap();
        });
    }
}
