//! Car lighting and garage camera numbers of the original game, with the
//! addresses they have in the reference executable (docs/exe-tables.md,
//! "Tuning constants").

use crate::exe_tables::records::{RigLight, Shot};
use glam::Vec3;

/// The in-game car light setup (0x7F3F00).
pub const IN_GAME_RIG: [RigLight; 3] = [
    RigLight { camera_relative: true, theta: 200.0, phi: 90.0, colour: Vec3::new(1.0, 1.0, 1.0), intensity: 0.5 },
    RigLight { camera_relative: true, theta: 42.0, phi: 33.0, colour: Vec3::new(1.0, 1.0, 1.0), intensity: 0.35 },
    RigLight { camera_relative: true, theta: 219.0, phi: 10.0, colour: Vec3::new(1.0, 1.0, 1.0), intensity: 0.1 },
];

/// The default car light setup (0x7F3E10).
pub const DEFAULT_RIG: [RigLight; 3] = [
    RigLight { camera_relative: true, theta: 149.0, phi: 100.0, colour: Vec3::new(1.0, 1.0, 1.0), intensity: 0.2 },
    RigLight { camera_relative: true, theta: 303.0, phi: 36.0, colour: Vec3::new(1.0, 1.0, 1.0), intensity: 0.2 },
    RigLight { camera_relative: true, theta: 76.0, phi: 22.0, colour: Vec3::new(1.0, 1.0, 1.0), intensity: 0.2 },
];

/// The garage's part views, cars then SUVs: the slot and its view (0x7F4608: two tables of 0x31 records of 0x3C bytes, records 0..41).
pub const SLOT_SHOTS: [[(u32, Shot); 42]; 2] = [
    [
        (
            27,
            Shot {
                car_angle: 322.7604,
                steer: 26.0,
                fov: 15.0,
                target: Vec3::new(0.5997, 4.4816, 0.5994),
                eye: Vec3::new(-0.0602, -4.0, 1.299),
            },
        ),
        (
            1,
            Shot {
                car_angle: 249.739,
                steer: 21.6723,
                fov: 24.0,
                target: Vec3::new(-0.3362, 4.4816, 0.97),
                eye: Vec3::new(-0.0602, -4.0, 1.2033),
            },
        ),
        (
            2,
            Shot {
                car_angle: 73.9326,
                steer: 21.6723,
                fov: 25.0,
                target: Vec3::new(-0.3671, 4.4816, 1.0986),
                eye: Vec3::new(-0.0602, -4.0, 1.23),
            },
        ),
        (
            11,
            Shot {
                car_angle: 315.0,
                steer: 23.6221,
                fov: 23.6221,
                target: Vec3::new(-0.25, 4.806, 0.9993),
                eye: Vec3::new(-0.1344, -4.0, 1.6695),
            },
        ),
        (
            12,
            Shot {
                car_angle: 121.6744,
                steer: 21.6723,
                fov: 30.0,
                target: Vec3::new(0.1995, 4.4816, 1.1999),
                eye: Vec3::new(0.0, -4.0, 1.6052),
            },
        ),
        (
            6,
            Shot {
                car_angle: 296.5652,
                steer: 21.6723,
                fov: 32.0,
                target: Vec3::new(0.1, 4.6072, 1.0798),
                eye: Vec3::new(-0.0602, -4.0, 2.4059),
            },
        ),
        (
            9,
            Shot {
                car_angle: 244.5537,
                steer: 21.6723,
                fov: 34.0,
                target: Vec3::new(-0.3342, 4.6072, 1.1198),
                eye: Vec3::new(-0.1344, -4.0, 2.8054),
            },
        ),
        (
            7,
            Shot {
                car_angle: 299.9696,
                steer: 21.6723,
                fov: 20.0,
                target: Vec3::new(-0.1343, 4.0735, 1.6295),
                eye: Vec3::new(-0.1, -4.0, 3.6241),
            },
        ),
        (
            29,
            Shot {
                car_angle: 31.2175,
                steer: -24.0,
                fov: 26.0,
                target: Vec3::new(0.1654, 4.6072, 1.14),
                eye: Vec3::new(0.0, -4.0, 1.0992),
            },
        ),
        (
            13,
            Shot {
                car_angle: 254.8107,
                steer: 22.0,
                fov: 21.0,
                target: Vec3::new(-0.1962, 4.161, 1.2217),
                eye: Vec3::new(0.0, -4.0, 3.0995),
            },
        ),
        (
            16,
            Shot {
                car_angle: 97.2541,
                steer: 16.0,
                fov: 26.0,
                target: Vec3::new(0.0, 4.6072, 1.0498),
                eye: Vec3::new(0.0, -4.0, 1.2992),
            },
        ),
        (
            34,
            Shot {
                car_angle: 90.0,
                steer: 1.0,
                fov: 20.0,
                target: Vec3::new(0.0, 4.4816, 0.8986),
                eye: Vec3::new(-0.0602, -2.0, 3.9993),
            },
        ),
        (
            154,
            Shot {
                car_angle: 295.1511,
                steer: 16.0,
                fov: 30.0,
                target: Vec3::new(0.075, 4.491, 1.0589),
                eye: Vec3::new(0.0, -4.0, 1.2),
            },
        ),
        (
            155,
            Shot {
                car_angle: 254.8107,
                steer: 22.0,
                fov: 21.0,
                target: Vec3::new(-0.1962, 4.161, 1.2217),
                eye: Vec3::new(0.0, -4.0, 3.0995),
            },
        ),
        (
            156,
            Shot {
                car_angle: 295.1511,
                steer: 16.0,
                fov: 30.0,
                target: Vec3::new(0.0, 4.491, 0.959),
                eye: Vec3::new(0.0, -4.0, 1.2),
            },
        ),
        (
            157,
            Shot {
                car_angle: 74.7915,
                steer: -16.0,
                fov: 20.0,
                target: Vec3::new(-0.4, 4.491, 0.9585),
                eye: Vec3::new(0.0, -4.0, 4.1991),
            },
        ),
        (
            153,
            Shot {
                car_angle: 55.4159,
                steer: 16.0,
                fov: 32.0,
                target: Vec3::new(-0.21, 4.277, 1.1451),
                eye: Vec3::new(0.0, -4.0, 2.3997),
            },
        ),
        (
            14,
            Shot {
                car_angle: 262.9463,
                steer: -15.0,
                fov: 25.0,
                target: Vec3::new(-0.096, 4.161, 0.9499),
                eye: Vec3::new(0.0, -4.0, 1.2),
            },
        ),
        (
            15,
            Shot {
                car_angle: 104.0,
                steer: 21.6723,
                fov: 26.5,
                target: Vec3::new(0.25, 4.45, 1.0596),
                eye: Vec3::new(0.0, -4.0, 1.2998),
            },
        ),
        (
            53,
            Shot {
                car_angle: 264.2639,
                steer: 16.0,
                fov: 14.0,
                target: Vec3::new(0.0, 4.0, 1.4945),
                eye: Vec3::new(0.0, -4.0, 2.799),
            },
        ),
        (
            54,
            Shot {
                car_angle: 83.5174,
                steer: -16.0,
                fov: 14.0,
                target: Vec3::new(-0.03, 4.731, 1.2998),
                eye: Vec3::new(0.0, -4.0, 3.1994),
            },
        ),
        (
            55,
            Shot {
                car_angle: 149.3824,
                steer: -16.0,
                fov: 16.0,
                target: Vec3::new(-0.6693, 4.092, 0.89),
                eye: Vec3::new(0.0, -4.0, 1.2),
            },
        ),
        (
            56,
            Shot {
                car_angle: 325.3438,
                steer: 16.0,
                fov: 16.0,
                target: Vec3::new(-0.4698, 4.092, 0.89),
                eye: Vec3::new(0.0, -4.0, 1.2),
            },
        ),
        (
            57,
            Shot {
                car_angle: 209.6176,
                steer: 10.0,
                fov: 15.0,
                target: Vec3::new(1.3303, 4.646, 1.1),
                eye: Vec3::new(0.09996, -4.0, 1.2),
            },
        ),
        (
            58,
            Shot {
                car_angle: 321.6277,
                steer: 10.0,
                fov: 15.0,
                target: Vec3::new(-1.1002, 4.646, 1.1),
                eye: Vec3::new(0.09996, -4.0, 1.2),
            },
        ),
        (
            52,
            Shot {
                car_angle: 259.8524,
                steer: -20.0,
                fov: 20.0,
                target: Vec3::new(-0.1, 4.184, 1.0996),
                eye: Vec3::new(0.0, -4.0, 4.6987),
            },
        ),
        (
            63,
            Shot {
                car_angle: 310.8121,
                steer: 16.0,
                fov: 32.0,
                target: Vec3::new(0.2, 4.4816, 1.0499),
                eye: Vec3::new(0.2, -4.0, 3.0026),
            },
        ),
        (
            69,
            Shot {
                car_angle: 121.6744,
                steer: 21.6723,
                fov: 30.0,
                target: Vec3::new(0.1995, 4.4816, 1.1999),
                eye: Vec3::new(0.0, -4.0, 1.6052),
            },
        ),
        (
            73,
            Shot {
                car_angle: 31.2175,
                steer: -24.0,
                fov: 26.0,
                target: Vec3::new(0.1654, 4.6072, 1.0399),
                eye: Vec3::new(0.0, -4.0, 1.2992),
            },
        ),
        (
            71,
            Shot {
                car_angle: 97.2541,
                steer: 24.0,
                fov: 26.0,
                target: Vec3::new(0.1999, 4.6072, 0.9499),
                eye: Vec3::new(0.0, -4.0, 1.2992),
            },
        ),
        (
            68,
            Shot {
                car_angle: 254.8107,
                steer: 22.0,
                fov: 21.0,
                target: Vec3::new(-0.1962, 4.161, 1.022),
                eye: Vec3::new(0.0, -4.0, 3.0995),
            },
        ),
        (
            70,
            Shot {
                car_angle: 322.7604,
                steer: 26.0,
                fov: 15.0,
                target: Vec3::new(0.5997, 4.4816, 0.5994),
                eye: Vec3::new(-0.0602, -4.0, 1.299),
            },
        ),
        (
            64,
            Shot {
                car_angle: 0.0,
                steer: 16.0,
                fov: 35.0,
                target: Vec3::new(-0.07, 4.0, 1.2),
                eye: Vec3::new(0.0, -4.0, 1.2),
            },
        ),
        (
            93,
            Shot {
                car_angle: 239.3335,
                steer: -24.0,
                fov: 35.0,
                target: Vec3::new(-0.1001, 4.0, 1.1),
                eye: Vec3::new(0.0, -4.0, 1.39992),
            },
        ),
        (
            94,
            Shot {
                car_angle: 244.5537,
                steer: 21.6723,
                fov: 34.0,
                target: Vec3::new(-0.3342, 4.6072, 1.1198),
                eye: Vec3::new(-0.1344, -4.0, 2.8054),
            },
        ),
        (
            95,
            Shot {
                car_angle: 0.0,
                steer: 16.0,
                fov: 37.0,
                target: Vec3::new(-0.1001, 4.0, 1.0),
                eye: Vec3::new(0.0, -4.0, 1.2),
            },
        ),
        (
            96,
            Shot {
                car_angle: 90.0,
                steer: 1.0,
                fov: 20.0,
                target: Vec3::new(0.0, 4.4816, 0.8986),
                eye: Vec3::new(-0.0602, -4.0, 6.0286),
            },
        ),
        (
            159,
            Shot {
                car_angle: 262.9463,
                steer: -15.0,
                fov: 25.0,
                target: Vec3::new(-0.096, 4.161, 0.9499),
                eye: Vec3::new(0.0, -4.0, 1.2),
            },
        ),
        (
            160,
            Shot {
                car_angle: 247.9781,
                steer: 20.0,
                fov: 32.0,
                target: Vec3::new(0.0299, 4.0, 1.2998),
                eye: Vec3::new(0.0, -4.0, 1.2),
            },
        ),
        (
            161,
            Shot {
                car_angle: 305.7729,
                steer: 0.0,
                fov: 32.0,
                target: Vec3::new(0.0299, 4.0, 1.19972),
                eye: Vec3::new(0.0, -4.0, 1.3997),
            },
        ),
        (
            162,
            Shot {
                car_angle: 295.4272,
                steer: 20.0,
                fov: 30.0,
                target: Vec3::new(0.09976, 4.0, 1.19992),
                eye: Vec3::new(0.0, -4.0, 1.2),
            },
        ),
        (
            32,
            Shot {
                car_angle: 254.6261,
                steer: -25.0,
                fov: 20.0,
                target: Vec3::new(-0.2008, 4.0, 1.26),
                eye: Vec3::new(0.0, -4.0, 2.0998),
            },
        ),
    ],
    [
        (
            27,
            Shot {
                car_angle: 322.7604,
                steer: 26.0,
                fov: 15.0,
                target: Vec3::new(0.5997, 4.4816, 0.5994),
                eye: Vec3::new(-0.0602, -4.0, 1.299),
            },
        ),
        (
            1,
            Shot {
                car_angle: 249.739,
                steer: 21.6723,
                fov: 30.0,
                target: Vec3::new(-0.3362, 4.4816, 1.2698),
                eye: Vec3::new(-0.0602, -4.0, 1.2033),
            },
        ),
        (
            2,
            Shot {
                car_angle: 73.9326,
                steer: 21.6723,
                fov: 34.0,
                target: Vec3::new(-0.3671, 4.4816, 1.3985),
                eye: Vec3::new(-0.0602, -4.0, 1.23),
            },
        ),
        (
            11,
            Shot {
                car_angle: 315.0,
                steer: 23.6221,
                fov: 27.0,
                target: Vec3::new(-0.25, 4.806, 1.05),
                eye: Vec3::new(-0.1344, -4.0, 1.6695),
            },
        ),
        (
            12,
            Shot {
                car_angle: 121.6744,
                steer: 24.6723,
                fov: 38.0,
                target: Vec3::new(0.3991, 4.4816, 1.5996),
                eye: Vec3::new(0.0, -4.0, 1.6052),
            },
        ),
        (
            6,
            Shot {
                car_angle: 296.5652,
                steer: 21.6723,
                fov: 37.0,
                target: Vec3::new(0.1, 4.6072, 1.3796),
                eye: Vec3::new(-0.0602, -4.0, 2.4059),
            },
        ),
        (
            9,
            Shot {
                car_angle: 244.5537,
                steer: 21.6723,
                fov: 38.0,
                target: Vec3::new(-0.3342, 4.6072, 1.4197),
                eye: Vec3::new(-0.1344, -4.0, 2.8054),
            },
        ),
        (
            7,
            Shot {
                car_angle: 299.9696,
                steer: 21.6723,
                fov: 23.0,
                target: Vec3::new(-0.1343, 4.0735, 2.0292),
                eye: Vec3::new(-0.1, -4.0, 3.6241),
            },
        ),
        (
            29,
            Shot {
                car_angle: 31.2175,
                steer: -24.0,
                fov: 33.0,
                target: Vec3::new(-0.0347, 4.6072, 1.3398),
                eye: Vec3::new(0.0, -4.0, 1.0992),
            },
        ),
        (
            13,
            Shot {
                car_angle: 254.8107,
                steer: -22.0,
                fov: 25.0,
                target: Vec3::new(-0.2962, 4.161, 1.2217),
                eye: Vec3::new(0.0, -4.0, 3.6992),
            },
        ),
        (
            16,
            Shot {
                car_angle: 97.2541,
                steer: 16.0,
                fov: 34.0,
                target: Vec3::new(0.0, 4.6072, 1.4497),
                eye: Vec3::new(0.0, -4.0, 1.2992),
            },
        ),
        (
            34,
            Shot {
                car_angle: 90.0,
                steer: 1.0,
                fov: 25.0,
                target: Vec3::new(0.0, 4.4816, 1.1983),
                eye: Vec3::new(-0.0602, -4.0, 2.0274),
            },
        ),
        (
            154,
            Shot {
                car_angle: 295.1511,
                steer: 25.0,
                fov: 35.0,
                target: Vec3::new(0.075, 4.491, 1.25884),
                eye: Vec3::new(0.0, -4.0, 1.2),
            },
        ),
        (
            155,
            Shot {
                car_angle: 254.8107,
                steer: -22.0,
                fov: 25.0,
                target: Vec3::new(-0.2962, 4.161, 1.2217),
                eye: Vec3::new(0.0, -4.0, 3.6992),
            },
        ),
        (
            156,
            Shot {
                car_angle: 295.1511,
                steer: 16.0,
                fov: 30.0,
                target: Vec3::new(0.0, 4.491, 0.959),
                eye: Vec3::new(0.0, -4.0, 1.2),
            },
        ),
        (
            157,
            Shot {
                car_angle: 74.7915,
                steer: -16.0,
                fov: 26.0,
                target: Vec3::new(-0.4003, 4.491, 1.2583),
                eye: Vec3::new(0.0, -4.0, 2.2984),
            },
        ),
        (
            153,
            Shot {
                car_angle: 55.4159,
                steer: 16.0,
                fov: 38.0,
                target: Vec3::new(-0.21, 4.277, 1.445),
                eye: Vec3::new(0.0, -4.0, 2.3997),
            },
        ),
        (
            14,
            Shot {
                car_angle: 262.9463,
                steer: 24.0,
                fov: 31.0,
                target: Vec3::new(-0.096, 4.161, 1.3498),
                eye: Vec3::new(0.0, -4.0, 1.2),
            },
        ),
        (
            15,
            Shot {
                car_angle: 104.0,
                steer: 24.6723,
                fov: 34.0,
                target: Vec3::new(0.25, 4.45, 1.4594),
                eye: Vec3::new(0.0, -4.0, 1.2998),
            },
        ),
        (
            53,
            Shot {
                car_angle: 264.2639,
                steer: 16.0,
                fov: 19.0,
                target: Vec3::new(-0.1, 4.0, 1.6944),
                eye: Vec3::new(0.0, -4.0, 2.799),
            },
        ),
        (
            54,
            Shot {
                car_angle: 83.5174,
                steer: -16.0,
                fov: 20.0,
                target: Vec3::new(-0.23, 4.731, 1.3998),
                eye: Vec3::new(0.0, -4.0, 3.1994),
            },
        ),
        (
            55,
            Shot {
                car_angle: 149.3824,
                steer: -24.0,
                fov: 17.0,
                target: Vec3::new(-0.6693, 4.092, 1.08996),
                eye: Vec3::new(0.0, -4.0, 1.2),
            },
        ),
        (
            56,
            Shot {
                car_angle: 325.3438,
                steer: 24.0,
                fov: 16.0,
                target: Vec3::new(0.03004, 4.092, 1.08988),
                eye: Vec3::new(0.0, -4.0, 1.2),
            },
        ),
        (
            57,
            Shot {
                car_angle: 209.6176,
                steer: 10.0,
                fov: 15.0,
                target: Vec3::new(1.3303, 4.646, 1.1),
                eye: Vec3::new(0.09996, -4.0, 1.2),
            },
        ),
        (
            58,
            Shot {
                car_angle: 321.6277,
                steer: 10.0,
                fov: 15.0,
                target: Vec3::new(-1.20028, 4.646, 1.1),
                eye: Vec3::new(0.09996, -4.0, 1.2),
            },
        ),
        (
            52,
            Shot {
                car_angle: 259.8524,
                steer: -20.0,
                fov: 22.0,
                target: Vec3::new(-0.1, 4.184, 1.0996),
                eye: Vec3::new(0.0, -4.0, 5.3985),
            },
        ),
        (
            63,
            Shot {
                car_angle: 310.8121,
                steer: 24.0,
                fov: 39.0,
                target: Vec3::new(0.2, 4.4816, 1.3498),
                eye: Vec3::new(0.2, -4.0, 3.0026),
            },
        ),
        (
            69,
            Shot {
                car_angle: 121.6744,
                steer: 24.6723,
                fov: 38.0,
                target: Vec3::new(0.3991, 4.4816, 1.49952),
                eye: Vec3::new(0.0, -4.0, 1.6052),
            },
        ),
        (
            73,
            Shot {
                car_angle: 31.2175,
                steer: -24.0,
                fov: 33.0,
                target: Vec3::new(-0.0347, 4.6072, 1.3398),
                eye: Vec3::new(0.0, -4.0, 1.0992),
            },
        ),
        (
            71,
            Shot {
                car_angle: 97.2541,
                steer: 16.0,
                fov: 35.0,
                target: Vec3::new(0.0, 4.6072, 1.4),
                eye: Vec3::new(0.0, -4.0, 1.2992),
            },
        ),
        (
            68,
            Shot {
                car_angle: 254.8107,
                steer: -22.0,
                fov: 25.0,
                target: Vec3::new(-0.2962, 4.161, 1.2217),
                eye: Vec3::new(0.0, -4.0, 3.6992),
            },
        ),
        (
            70,
            Shot {
                car_angle: 322.7604,
                steer: 26.0,
                fov: 15.0,
                target: Vec3::new(0.5997, 4.4816, 0.5994),
                eye: Vec3::new(-0.0602, -4.0, 1.299),
            },
        ),
        (
            64,
            Shot {
                car_angle: 0.0,
                steer: 25.0,
                fov: 40.0,
                target: Vec3::new(-0.1701, 4.0, 1.2),
                eye: Vec3::new(0.0, -4.0, 1.2),
            },
        ),
        (
            93,
            Shot {
                car_angle: 239.3335,
                steer: -24.0,
                fov: 35.0,
                target: Vec3::new(-0.1001, 4.0, 1.1),
                eye: Vec3::new(0.0, -4.0, 1.39992),
            },
        ),
        (
            94,
            Shot {
                car_angle: 244.5537,
                steer: 21.6723,
                fov: 38.0,
                target: Vec3::new(-0.3342, 4.6072, 1.4197),
                eye: Vec3::new(-0.1344, -4.0, 2.8054),
            },
        ),
        (
            95,
            Shot {
                car_angle: 0.0,
                steer: 16.0,
                fov: 37.0,
                target: Vec3::new(-0.1001, 4.0, 1.0),
                eye: Vec3::new(0.0, -4.0, 1.2),
            },
        ),
        (
            96,
            Shot {
                car_angle: 74.7915,
                steer: -16.0,
                fov: 26.0,
                target: Vec3::new(-0.4003, 4.491, 1.2583),
                eye: Vec3::new(0.0, -4.0, 2.2984),
            },
        ),
        (
            159,
            Shot {
                car_angle: 262.9463,
                steer: 24.0,
                fov: 31.0,
                target: Vec3::new(-0.096, 4.161, 1.3498),
                eye: Vec3::new(0.0, -4.0, 1.2),
            },
        ),
        (
            160,
            Shot {
                car_angle: 247.9781,
                steer: -20.0,
                fov: 34.0,
                target: Vec3::new(0.0299, 4.0, 1.2998),
                eye: Vec3::new(0.0, -4.0, 1.2),
            },
        ),
        (
            161,
            Shot {
                car_angle: 305.7729,
                steer: 0.0,
                fov: 37.0,
                target: Vec3::new(0.0299, 4.0, 1.4996),
                eye: Vec3::new(0.0, -4.0, 1.3997),
            },
        ),
        (
            162,
            Shot {
                car_angle: 295.4272,
                steer: 20.0,
                fov: 33.0,
                target: Vec3::new(0.09976, 4.0, 1.29992),
                eye: Vec3::new(0.0, -4.0, 1.2),
            },
        ),
        (
            32,
            Shot {
                car_angle: 254.6261,
                steer: -25.0,
                fov: 25.0,
                target: Vec3::new(-0.3008, 4.0, 1.26),
                eye: Vec3::new(0.0, -4.0, 2.0998),
            },
        ),
    ],
];

/// The vinyl views, cars then SUVs (0x7F5D40, 7 records of 0x3C a class).
pub const VINYL_SHOTS: [[Shot; 7]; 2] = [
    [
        Shot {
            car_angle: 254.8107,
            steer: 22.0,
            fov: 21.0,
            target: Vec3::new(-0.1962, 4.161, 1.0992),
            eye: Vec3::new(0.0, -4.0, 5.4973),
        },
        Shot {
            car_angle: 317.7093,
            steer: 24.0,
            fov: 32.0,
            target: Vec3::new(0.0, 4.4607, 1.1798),
            eye: Vec3::new(-0.0602, -4.0, 2.4059),
        },
        Shot {
            car_angle: 222.5111,
            steer: 24.0,
            fov: 32.0,
            target: Vec3::new(-0.1671, 4.4607, 1.1798),
            eye: Vec3::new(-0.0602, -4.0, 2.4059),
        },
        Shot {
            car_angle: 317.7093,
            steer: 24.0,
            fov: 32.0,
            target: Vec3::new(0.0, 4.4607, 1.1798),
            eye: Vec3::new(-0.0602, -4.0, 2.4059),
        },
        Shot {
            car_angle: 0.0,
            steer: 16.0,
            fov: 35.0,
            target: Vec3::new(-0.07, 4.0, 1.2),
            eye: Vec3::new(0.0, -4.0, 1.2),
        },
        Shot {
            car_angle: 317.7093,
            steer: 24.0,
            fov: 32.0,
            target: Vec3::new(0.0, 4.4607, 1.1798),
            eye: Vec3::new(-0.0602, -4.0, 2.4059),
        },
        Shot {
            car_angle: 317.7093,
            steer: 24.0,
            fov: 32.0,
            target: Vec3::new(0.0, 4.4607, 1.1798),
            eye: Vec3::new(-0.0602, -4.0, 2.4059),
        },
    ],
    [
        Shot {
            car_angle: 254.8107,
            steer: 22.0,
            fov: 28.0,
            target: Vec3::new(-0.2962, 4.161, 1.4216),
            eye: Vec3::new(0.0, -4.0, 4.6989),
        },
        Shot {
            car_angle: 317.7093,
            steer: 28.0,
            fov: 36.0,
            target: Vec3::new(0.0999, 4.4607, 1.3797),
            eye: Vec3::new(-0.0602, -4.0, 2.4059),
        },
        Shot {
            car_angle: 222.5111,
            steer: 28.0,
            fov: 38.0,
            target: Vec3::new(-0.1671, 4.4607, 1.4797),
            eye: Vec3::new(-0.0602, -4.0, 2.4059),
        },
        Shot {
            car_angle: 317.7093,
            steer: 28.0,
            fov: 36.0,
            target: Vec3::new(0.0999, 4.4607, 1.3797),
            eye: Vec3::new(-0.0602, -4.0, 2.4059),
        },
        Shot {
            car_angle: 0.0,
            steer: 25.0,
            fov: 40.0,
            target: Vec3::new(-0.1701, 4.0, 1.2),
            eye: Vec3::new(0.0, -4.0, 1.2),
        },
        Shot {
            car_angle: 317.7093,
            steer: 28.0,
            fov: 36.0,
            target: Vec3::new(0.0999, 4.4607, 1.3797),
            eye: Vec3::new(-0.0602, -4.0, 2.4059),
        },
        Shot {
            car_angle: 317.7093,
            steer: 28.0,
            fov: 36.0,
            target: Vec3::new(0.0999, 4.4607, 1.3797),
            eye: Vec3::new(-0.0602, -4.0, 2.4059),
        },
    ],
];

/// The SUV engine view (0x7F5D00).
pub const SUV_ENGINE_SHOT: Shot = Shot {
    car_angle: 199.5721,
    steer: -22.0,
    fov: 15.0,
    target: Vec3::new(-1.3348, 4.6072, 1.55),
    eye: Vec3::new(-0.1344, -4.0, 5.0),
};
