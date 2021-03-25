use std::f64::consts::PI;

use bevy::{
    asset::LoadState,
    diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin},
    gltf::{Gltf, GltfMesh},
    prelude::*,
    render::wireframe::{Wireframe, WireframePlugin},
    wgpu::{WgpuFeature, WgpuFeatures, WgpuOptions},
};
use rand::Rng;

use navmesh::*;

fn main() {
    App::build()
        .insert_resource(Msaa { samples: 4 })
        .insert_resource(ClearColor(Color::rgb(0., 0., 0.01)))
        .insert_resource(WgpuOptions {
            features: WgpuFeatures {
                // The Wireframe requires NonFillPolygonMode feature
                features: vec![WgpuFeature::NonFillPolygonMode],
            },
            ..Default::default()
        })
        .init_resource::<GltfHandles>()
        .add_plugins(DefaultPlugins)
        .add_plugin(WireframePlugin)
        .add_plugin(FrameTimeDiagnosticsPlugin::default())
        .add_startup_system(setup.system())
        .init_resource::<Option<NavMesh>>()
        .add_state(AppState::Setup)
        .add_system_set(SystemSet::on_update(AppState::Setup).with_system(check_textures.system()))
        .add_system_set(SystemSet::on_enter(AppState::Playing).with_system(setup_scene.system()))
        .add_system_set(
            SystemSet::on_update(AppState::Playing)
                .with_system(give_target.system())
                .with_system(move_object.system())
                .with_system(display_fps.system())
                .with_system(rotate_camera.system())
                .with_system(trigger_navmesh_visibility.system())
                .with_system(exit.system()),
        )
        .run();
}

#[derive(Clone, PartialEq, Eq, Debug, Hash)]
enum AppState {
    Setup,
    Playing,
}

#[derive(Default)]
struct GltfHandles {
    handles: Vec<Handle<Gltf>>,
}

struct FpsText;
struct Camera;

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut navmeshes: ResMut<GltfHandles>,
) {
    navmeshes.handles = vec![
        asset_server.load("meshes/my_level_nav.glb"),
        asset_server.load("meshes/my_level.glb"),
    ];

    commands.spawn_bundle(LightBundle {
        transform: Transform::from_xyz(0.0, 10.0, 0.0),
        light: Light {
            range: 40.0,
            intensity: 500.0,
            ..Default::default()
        },
        ..Default::default()
    });
    commands
        .spawn_bundle(PerspectiveCameraBundle {
            transform: Transform::from_xyz(0.0, 40.0, 0.1)
                .looking_at(Vec3::new(0.0, 0.3, 0.0), Vec3::Y),
            ..Default::default()
        })
        .insert(Camera);

    commands.spawn_bundle(UiCameraBundle::default());
    commands
        .spawn_bundle(TextBundle {
            style: Style {
                align_self: AlignSelf::FlexEnd,
                ..Default::default()
            },
            // Use `Text` directly
            text: Text {
                // Construct a `Vec` of `TextSection`s
                sections: vec![
                    TextSection {
                        value: "FPS: ".to_string(),
                        style: TextStyle {
                            font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                            font_size: 30.0,
                            color: Color::WHITE,
                        },
                    },
                    TextSection {
                        value: "".to_string(),
                        style: TextStyle {
                            font: asset_server.load("fonts/FiraMono-Medium.ttf"),
                            font_size: 30.0,
                            color: Color::GOLD,
                        },
                    },
                ],
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(FpsText);
}

fn display_fps(diagnostics: Res<Diagnostics>, mut query: Query<&mut Text, With<FpsText>>) {
    for mut text in query.iter_mut() {
        if let Some(fps) = diagnostics.get(FrameTimeDiagnosticsPlugin::FPS) {
            if let Some(average) = fps.average() {
                // Update the value of the second section
                text.sections[1].value = format!("{:.2}", average);
            }
        }
    }
}

fn check_textures(
    mut state: ResMut<State<AppState>>,
    navmeshes: ResMut<GltfHandles>,
    asset_server: Res<AssetServer>,
) {
    if let LoadState::Loaded =
        asset_server.get_group_load_state(navmeshes.handles.iter().map(|handle| handle.id))
    {
        state.set_next(AppState::Playing).unwrap();
    }
}

struct Path {
    current: Vec3,
    next: Vec<Vec3>,
}
struct Object;
struct Target;
struct Waiting(Timer);
struct NavMeshDisp;

fn setup_scene(
    mut commands: Commands,
    navmeshes: Res<GltfHandles>,
    gltfs: Res<Assets<Gltf>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut navmesh: ResMut<Option<NavMesh>>,
) {
    if let Some(gltf) = gltfs.get(&navmeshes.handles[1]) {
        commands.spawn_scene(gltf.default_scene.as_ref().unwrap().clone());
    }

    if let Some(gltf) = gltfs.get(&navmeshes.handles[0]) {
        let gltf_mesh_handle = gltf.meshes[0].clone();
        let gltf_mesh = gltf_meshes.get(gltf_mesh_handle).unwrap();
        let mesh_handle = gltf_mesh.primitives[0].mesh.clone();

        commands
            .spawn_bundle(PbrBundle {
                mesh: mesh_handle.clone(),
                material: materials.add(Color::ORANGE.into()),
                transform: Transform::from_xyz(0.0, -0.2, 0.0),
                ..Default::default()
            })
            .insert(Wireframe)
            .insert(NavMeshDisp);

        let mesh = meshes.get(mesh_handle).unwrap();

        *navmesh = Some(NavMesh::from_mesh(mesh));

        let mut x;
        let mut z;
        loop {
            x = rand::thread_rng().gen_range(-20.0..20.0);
            z = rand::thread_rng().gen_range(-20.0..20.0);
            if navmesh
                .as_ref()
                .unwrap()
                .point_in_mesh(Vec3::new(x, 0.0, z))
            {
                break;
            }
        }

        commands
            .spawn_bundle(PbrBundle {
                mesh: meshes.add(Mesh::from(shape::Cube { size: 0.7 })),
                material: materials.add(Color::BLUE.into()),
                transform: Transform::from_xyz(x, 0.0, z),
                ..Default::default()
            })
            .insert(Object)
            .insert(Waiting(Timer::from_seconds(1.0, false)));
    }
}

fn give_target(
    mut commands: Commands,
    mut object_query: Query<(Entity, &Transform, &mut Waiting)>,
    navmesh: Res<Option<NavMesh>>,
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, transform, mut waiting) in object_query.iter_mut() {
        if !waiting.0.tick(time.delta()).finished() {
            break;
        }

        let mut x;
        let mut z;
        loop {
            x = rand::thread_rng().gen_range(-20.0..20.0);
            z = rand::thread_rng().gen_range(-20.0..20.0);
            if navmesh
                .as_ref()
                .unwrap()
                .point_in_mesh(Vec3::new(x, 0.0, z))
            {
                break;
            }
        }

        let path = navmesh
            .as_ref()
            .unwrap()
            .path_from_to(transform.translation, Vec3::new(x, 0.0, z));

        if let Some((first, remaining)) = path.split_first() {
            let mut remaining = remaining.to_vec();
            remaining.reverse();
            commands.entity(entity).insert(Path {
                current: first.clone(),
                next: remaining,
            });
            commands.entity(entity).remove::<Waiting>();
            commands
                .spawn_bundle(PbrBundle {
                    mesh: meshes.add(Mesh::from(shape::Cube { size: 0.5 })),
                    material: materials.add(Color::RED.into()),
                    transform: Transform::from_xyz(x, 0.0, z),
                    ..Default::default()
                })
                .insert(Target);
        } else {
            commands.spawn_bundle(PbrBundle {
                mesh: meshes.add(Mesh::from(shape::Cube { size: 0.5 })),
                material: materials.add(Color::GREEN.into()),
                transform: Transform::from_xyz(x, 0.0, z),
                ..Default::default()
            });
        }
    }
}

fn move_object(
    mut commands: Commands,
    mut object_query: Query<(&mut Transform, &mut Path, Entity)>,
    target_query: Query<Entity, With<Target>>,
    time: Res<Time>,
) {
    for (mut transform, mut target, entity) in object_query.iter_mut() {
        let move_direction = target.current - transform.translation;
        transform.translation += move_direction.normalize() * time.delta_seconds() * 10.0;
        if transform.translation.distance(target.current) < 0.1 {
            if let Some(next) = target.next.pop() {
                target.current = next;
            } else {
                commands
                    .entity(entity)
                    .remove::<Path>()
                    .insert(Waiting(Timer::from_seconds(0.01, false)));
                for target in target_query.iter() {
                    commands.entity(target).despawn();
                }
            }
        }
    }
}

fn rotate_camera(time: Res<Time>, mut camera_query: Query<&mut Transform, With<Camera>>) {
    for mut camera in camera_query.iter_mut() {
        *camera = Transform::from_xyz(
            (time.seconds_since_startup() / (2.0 * PI) * 40.0 / 30.0).sin() as f32 * 20.0,
            40.0,
            (time.seconds_since_startup() / (2.0 * PI) * 40.0 / 30.0).cos() as f32 * 20.0,
        )
        .looking_at(Vec3::new(0.0, 0.3, 0.0), Vec3::Y);
    }
}

fn trigger_navmesh_visibility(
    mut query: Query<(&mut Visible, &mut Transform), With<NavMeshDisp>>,
    keyboard_input: ResMut<Input<KeyCode>>,
) {
    if keyboard_input.just_pressed(KeyCode::Space) {
        for (_visible, mut transform) in query.iter_mut() {
            transform.translation.y = -1.0 * transform.translation.y;
        }
        // https://github.com/bevyengine/bevy/issues/1717
        // for (mut visible, _transform) in query.iter_mut() {
        //     visible.is_visible = !visible.is_visible;
        // }
    }
}

fn exit(mut n: Local<u32>, mut aee: EventWriter<bevy::app::AppExit>) {
    if std::env::var("FAILFAST").is_ok() {
        *n += 1;
        if *n > 150 {
            aee.send(bevy::app::AppExit);
        }
    }
}
