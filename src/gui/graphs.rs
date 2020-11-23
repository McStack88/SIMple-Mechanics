use ggez::graphics::{self, Text};
use specs::prelude::*;
use specs::storage::BTreeStorage;
use specs::Component;

use std::collections::VecDeque;

use crate::{
    components::{self, Name},
    main_state::MainState,
    resources::{GraphMinMax, GraphPosData},
    RigidBody,
};
use graphics::{DrawMode, MeshBuilder, Rect, Scale, TextFragment};

// use csv;

pub trait Graph {
    fn draw(&self, builder: &mut MeshBuilder, color: graphics::Color, min_max: Option<(f32, f32)>);
}

pub enum PointShape {
    Ring,
    Dot,
    Square,
    Diamond,
}

pub trait LineGraph {
    fn add_val(&mut self, val: f32);
    fn points(&self) -> (&[[f32; 2]], &[[f32; 2]]);
    fn name(&self) -> String;
    fn shown(&self) -> bool;
    fn max_len(&self) -> usize;
    fn point_shape(&self) -> PointShape;
    fn access_field(rigid_body: &RigidBody) -> f32
    where
        Self: Sized;
}

impl Graph for dyn LineGraph {
    fn draw(
        &self,
        builder: &mut MeshBuilder,
        color: graphics::Color,
        midpoint_scale: Option<(f32, f32)>,
    ) {
        use std::f32::{INFINITY, NEG_INFINITY};

        let (s0, s1) = self.points();
        let (midpoint, scale_fac) = midpoint_scale.unwrap_or_else(|| {
            let (min, max) = s0
                .iter()
                .chain(s1.iter())
                .fold((INFINITY, NEG_INFINITY), |(min, max), [_, v]| {
                    (min.min(*v), max.max(*v))
                });
            let midpoint = (min + max) / 2.0;
            let scale_fac = 8.0 / (max - min).max(1.0 / 8.0);
            (midpoint, scale_fac)
        });

        let graph_len = s0.len() + s1.len();
        if graph_len >= 3 {
            // having a match statement inside of a loop that runs
            // hundreds to thousands of times per frame seems like a bad
            // idea, so save the draw function.
            // I'm not sure if this makes a significant performance
            // difference. Having to essentially declare the type signature
            // twice is kind of weird
            //
            // UPDATE: The number of points to draw is limited to 10 now so it's
            // probably a performance loss
            let mut draw_fn = match self.point_shape() {
                PointShape::Dot => Box::new(|builder: &mut MeshBuilder, point: [f32; 2]| {
                    builder.circle(DrawMode::fill(), point, 0.125, 0.0005, color);
                }) as Box<dyn FnMut(&mut MeshBuilder, [f32; 2])>,
                PointShape::Ring => Box::new(|builder: &mut MeshBuilder, point: [f32; 2]| {
                    builder.circle(DrawMode::stroke(0.05), point, 0.15, 0.001, color);
                })
                    as Box<dyn FnMut(&mut MeshBuilder, [f32; 2])>,
                PointShape::Square => Box::new(|builder: &mut MeshBuilder, point: [f32; 2]| {
                    builder.rectangle(
                        DrawMode::fill(),
                        Rect::new(point[0], point[1] - 0.1, 0.2, 0.2),
                        color,
                    );
                })
                    as Box<dyn FnMut(&mut MeshBuilder, [f32; 2])>,
                PointShape::Diamond => Box::new(|builder: &mut MeshBuilder, point: [f32; 2]| {
                    builder.circle(DrawMode::fill(), point, 0.2, 0.1, color);
                }),
            };

            let x_offset = {
                let self_len = {
                    let points = self.points();
                    points.0.len() + points.1.len()
                };
                let frac_completed = self_len as f32 / self.max_len() as f32;

                (1.0 - frac_completed) * 10.0
            };

            let mut transformed_points = s0
                .iter()
                .chain(s1.iter())
                .map(|[x, v]| [x_offset + dbg!(*x), 5.5 - (v - midpoint) * scale_fac])
                .collect::<Vec<[f32; 2]>>();

            transformed_points.pop();

            builder
                .line(transformed_points.as_slice(), 0.1, color)
                .unwrap();

            transformed_points
                .iter()
                .step_by((graph_len / 5).max(1))
                .for_each(|point| draw_fn(builder, *point));
        }
    }
}

macro_rules! create_linegraph {
    ($structname:ident, $name:expr, $point_shape:expr, $access_closure:expr) => {
        #[derive(Debug, Clone, Component)]
        #[storage(BTreeStorage)]
        pub struct $structname {
            pub data: VecDeque<[f32; 2]>,
            pub shown: bool,
            pub max_len: usize,
        }

        impl Default for $structname {
            fn default() -> Self {
                $structname {
                    data: VecDeque::with_capacity(60 * 10 / 4),
                    shown: true,
                    max_len: 60 * 5,
                }
            }
        }

        impl LineGraph for $structname {
            fn points(&self) -> (&[[f32; 2]], &[[f32; 2]]) {
                self.data.as_slices()
            }

            fn add_val(&mut self, val: f32) {
                let num_vals = self.data.len() + 1;
                let step_incr = 10.0 / self.max_len() as f32;

                self.data.iter_mut().enumerate().for_each(|(i, [x, _])| {
                    *x = step_incr * i as f32;
                });
                if num_vals < self.max_len {
                    self.data.push_back([10.0, val]);
                } else {
                    self.data.pop_front();
                    self.data.push_back([10.0, val]);
                }
            }

            fn name(&self) -> String {
                $name.to_string()
            }

            fn shown(&self) -> bool {
                self.shown
            }

            fn max_len(&self) -> usize {
                self.max_len
            }

            fn point_shape(&self) -> PointShape {
                $point_shape
            }

            fn access_field(rigid_body: &RigidBody) -> f32 {
                $access_closure(rigid_body)
            }
        }
    };
}

create_linegraph!(
    SpeedGraph,
    "Speed",
    PointShape::Square,
    |rigid_body: &RigidBody| rigid_body.velocity().linear.norm()
);
create_linegraph!(
    RotVelGraph,
    "Rotational Velocity",
    PointShape::Dot,
    |rigid_body: &RigidBody| rigid_body.velocity().angular
);
create_linegraph!(
    XPosGraph,
    "X Position",
    PointShape::Ring,
    |rigid_body: &RigidBody| rigid_body.position().translation.x
);
create_linegraph!(
    YPosGraph,
    "Y Position",
    PointShape::Ring,
    |rigid_body: &RigidBody| rigid_body.position().translation.y
);
create_linegraph!(
    XVelGraph,
    "X Velocity",
    PointShape::Diamond,
    |rigid_body: &RigidBody| rigid_body.velocity().linear.x
);
create_linegraph!(
    YVelGraph,
    "Y Velocity",
    PointShape::Diamond,
    |rigid_body: &RigidBody| rigid_body.velocity().linear.y
);
create_linegraph!(
    RotGraph,
    "Rotation",
    PointShape::Ring,
    |rigid_body: &RigidBody| rigid_body.position().rotation.angle()
);

impl<'a, 'b> MainState<'a, 'b> {
    pub fn draw_graphs(&self) -> ([Text; 3], MeshBuilder) {
        use specs::prelude::*;

        let mut builder = MeshBuilder::new();

        // let speed_graphs = self.world.read_storage::<SpeedGraph>();
        let colors = self.world.read_storage::<components::Color>();
        let GraphMinMax(min, max) = *self.world.fetch::<GraphMinMax>();
        let midpoint = (min + max) / 2.0;
        let scale_fac = 8.0 / (max - min).max(1.0 / 8.0);

        let mut first_iter = true;
        macro_rules! draw_graphtype {
            ( $graphtype:ident ) => {
                let graph_storages = self.world.read_storage::<$graphtype>();
                (&graph_storages, &colors)
                    .join()
                    .for_each(|(graph, color)| {
                        if graph.shown {
                            if first_iter {
                                first_iter = false;
                                draw_graph_frame(&mut builder);
                            }
                            Graph::draw(
                                graph as &dyn LineGraph,
                                &mut builder,
                                color.0,
                                Some((midpoint, scale_fac)),
                            );
                        }
                    });
            };
        }

        draw_graphtype!(SpeedGraph);
        draw_graphtype!(RotVelGraph);
        draw_graphtype!(XVelGraph);
        draw_graphtype!(YVelGraph);
        draw_graphtype!(XPosGraph);
        draw_graphtype!(YPosGraph);
        draw_graphtype!(RotGraph);

        let max_text = graphics::Text::new(
            TextFragment::new(format!("{0:.3}", max)).scale(Scale::uniform(25.0)),
        );
        let mid_text = graphics::Text::new(
            TextFragment::new(format!("{0:.3}", midpoint)).scale(Scale::uniform(25.0)),
        );
        let min_text = graphics::Text::new(
            TextFragment::new(format!("{0:.3}", min)).scale(Scale::uniform(25.0)),
        );

        ([max_text, mid_text, min_text], builder)
    }

    pub fn graph_grab_rect(&self) -> Rect {
        let graph_rect = self.world.fetch::<GraphPosData>().0;
        let scale_fac = graph_rect.w / 10.0;
        graphics::Rect::new(
            graph_rect.x + (9.5 * scale_fac),
            graph_rect.y + (9.5 * scale_fac),
            0.5 * scale_fac,
            0.5 * scale_fac,
        )
    }

    pub fn serialize_graphs_to_csv(
        &self,
        filename: impl AsRef<std::path::Path> + std::clone::Clone,
    ) {
        struct Column {
            name: String,
            data: Vec<f32>,
        }

        let mut columns: Vec<Column> = Vec::new();
        let names = self.world.read_storage::<Name>();

        macro_rules! add_linegraph_columns {
            ($graphtype:ty) => {
                let graphs = self.world.read_storage::<$graphtype>();
                (&graphs, &names).join().for_each(|(graph, name)| {
                    let data = {
                        let (s0, s1) = graph.points();
                        s0.iter()
                            .chain(s1.iter())
                            .map(|[_, val]| *val)
                            .collect::<Vec<f32>>()
                    };
                    let column = Column {
                        name: format!("{} {}", name.0, graph.name()),
                        data,
                    };

                    columns.push(column);
                });
            };
        }

        add_linegraph_columns!(SpeedGraph);
        add_linegraph_columns!(RotGraph);
        add_linegraph_columns!(XPosGraph);
        add_linegraph_columns!(YPosGraph);
        add_linegraph_columns!(XVelGraph);
        add_linegraph_columns!(YVelGraph);
        add_linegraph_columns!(RotVelGraph);

        let mut writer = csv::Writer::from_path(filename).expect("error creating csv writer");

        let names = columns
            .iter()
            .map(|column| column.name.clone())
            .collect::<Vec<String>>();
        writer
            .write_record(&names)
            .expect("Error writing header record");

        // i hate this a little bit
        // CSVs work row by row (because files are top to bottom)
        // Because of that, I make an iterator for each column
        // and call .next() on each iterator at each row to get the column
        // value
        type RecordIter<'a> = Box<std::slice::Iter<'a, f32>>;
        let mut iter_ls = columns
            .iter()
            .map(|column| Box::new(column.data.iter()) as RecordIter)
            .collect::<Vec<RecordIter>>();

        loop {
            let record = iter_ls
                .iter_mut()
                .map(|record_iter| {
                    record_iter
                        .next()
                        .map(|val| format!("{:.3}", val))
                        .unwrap_or_else(|| "".to_string())
                })
                .collect::<Vec<String>>();

            if record.iter().all(|val| val == &"".to_string()) {
                break;
            }

            writer
                .write_record(record.as_slice())
                .expect("Error writing data record");
        }
    }
}

fn draw_graph_frame(builder: &mut MeshBuilder) {
    use ggez::graphics::{BLACK, WHITE};
    builder.rectangle(
        DrawMode::stroke(0.1),
        Rect::new(0.0, 0.0, 10.0, 10.0),
        WHITE,
    );
    builder.rectangle(DrawMode::fill(), Rect::new(0.0, 0.0, 10.0, 10.0), BLACK);
    builder.rectangle(
        DrawMode::fill(),
        Rect::new(9.5, 9.5, 0.5, 0.5),
        graphics::Color::new(0.45, 0.6, 0.85, 1.0),
    );
}
