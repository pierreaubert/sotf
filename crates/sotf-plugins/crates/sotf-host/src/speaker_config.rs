// ============================================================================
// Speaker Configuration Module
// ============================================================================
//
// Standard speaker positions based on ITU-R BS.775 and ITU-R BS.2051 specs
// Azimuth: 0° = front, +90° = left, -90° = right, ±180° = back
// Elevation: 0° = ear level, +90° = overhead

use serde::{Deserialize, Serialize};

/// Speaker position in 3D space using spherical coordinates
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SpeakerPosition {
    /// Channel label (e.g., "FL", "C", "TFL")
    pub label: &'static str,
    /// Full name (e.g., "Front Left", "Center")
    pub name: &'static str,
    /// Horizontal angle in degrees (-180 to +180)
    pub azimuth: f32,
    /// Vertical angle in degrees (0 to 90)
    pub elevation: f32,
    /// Channel index in output array
    pub channel: usize,
    /// True if this is the LFE channel
    pub is_lfe: bool,
}

/// Speaker configuration preset
#[derive(Debug, Clone, PartialEq)]
pub struct SpeakerConfig {
    /// Configuration ID (e.g., "5.1", "7.1.4")
    pub id: &'static str,
    /// Display name
    pub name: &'static str,
    /// Description
    pub description: &'static str,
    /// Total number of channels including LFE
    pub total_channels: usize,
    /// Speaker positions
    pub speakers: &'static [SpeakerPosition],
    /// Channel groupings for level meters
    pub meter_groups: &'static [MeterGroupSpec],
}

// ============================================================================
// Meter Group Definitions
// ============================================================================

/// Channel info for meter display (static definition)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeterChannelSpec {
    /// Channel index in output array
    pub index: usize,
    /// Short label (e.g., "L", "R", "C")
    pub label: &'static str,
    /// Display characters for vertical rendering (e.g., ["S", "L"] for "SL")
    pub display_chars: &'static [&'static str],
}

/// Meter group specification (static definition)
#[derive(Debug, Clone, PartialEq)]
pub struct MeterGroupSpec {
    /// Group name (e.g., "L/R", "Center", "Surrounds")
    pub name: &'static str,
    /// Channels in this group
    pub channels: &'static [MeterChannelSpec],
}

// Meter group definitions for each configuration

const METER_GROUPS_1_0: &[MeterGroupSpec] = &[MeterGroupSpec {
    name: "Mono",
    channels: &[MeterChannelSpec {
        index: 0,
        label: "M",
        display_chars: &["M"],
    }],
}];

const METER_GROUPS_2_0: &[MeterGroupSpec] = &[MeterGroupSpec {
    name: "L/R",
    channels: &[
        MeterChannelSpec {
            index: 0,
            label: "L",
            display_chars: &["L"],
        },
        MeterChannelSpec {
            index: 1,
            label: "R",
            display_chars: &["R"],
        },
    ],
}];

const METER_GROUPS_2_1: &[MeterGroupSpec] = &[
    MeterGroupSpec {
        name: "L/R",
        channels: &[
            MeterChannelSpec {
                index: 0,
                label: "L",
                display_chars: &["L"],
            },
            MeterChannelSpec {
                index: 1,
                label: "R",
                display_chars: &["R"],
            },
        ],
    },
    MeterGroupSpec {
        name: "LFE",
        channels: &[MeterChannelSpec {
            index: 2,
            label: "LFE",
            display_chars: &["L", "F", "E"],
        }],
    },
];

const METER_GROUPS_5_0: &[MeterGroupSpec] = &[
    MeterGroupSpec {
        name: "L/R",
        channels: &[
            MeterChannelSpec {
                index: 0,
                label: "L",
                display_chars: &["L"],
            },
            MeterChannelSpec {
                index: 1,
                label: "R",
                display_chars: &["R"],
            },
        ],
    },
    MeterGroupSpec {
        name: "Center",
        channels: &[MeterChannelSpec {
            index: 2,
            label: "C",
            display_chars: &["C"],
        }],
    },
    MeterGroupSpec {
        name: "Surrounds",
        channels: &[
            MeterChannelSpec {
                index: 3,
                label: "SL",
                display_chars: &["S", "L"],
            },
            MeterChannelSpec {
                index: 4,
                label: "SR",
                display_chars: &["S", "R"],
            },
        ],
    },
];

const METER_GROUPS_5_1: &[MeterGroupSpec] = &[
    MeterGroupSpec {
        name: "L/R",
        channels: &[
            MeterChannelSpec {
                index: 0,
                label: "L",
                display_chars: &["L"],
            },
            MeterChannelSpec {
                index: 1,
                label: "R",
                display_chars: &["R"],
            },
        ],
    },
    MeterGroupSpec {
        name: "Center",
        channels: &[MeterChannelSpec {
            index: 2,
            label: "C",
            display_chars: &["C"],
        }],
    },
    MeterGroupSpec {
        name: "LFE",
        channels: &[MeterChannelSpec {
            index: 3,
            label: "LFE",
            display_chars: &["L", "F", "E"],
        }],
    },
    MeterGroupSpec {
        name: "Surrounds",
        channels: &[
            MeterChannelSpec {
                index: 4,
                label: "SL",
                display_chars: &["S", "L"],
            },
            MeterChannelSpec {
                index: 5,
                label: "SR",
                display_chars: &["S", "R"],
            },
        ],
    },
];

const METER_GROUPS_7_1: &[MeterGroupSpec] = &[
    MeterGroupSpec {
        name: "L/R",
        channels: &[
            MeterChannelSpec {
                index: 0,
                label: "L",
                display_chars: &["L"],
            },
            MeterChannelSpec {
                index: 1,
                label: "R",
                display_chars: &["R"],
            },
        ],
    },
    MeterGroupSpec {
        name: "Center",
        channels: &[MeterChannelSpec {
            index: 2,
            label: "C",
            display_chars: &["C"],
        }],
    },
    MeterGroupSpec {
        name: "LFE",
        channels: &[MeterChannelSpec {
            index: 3,
            label: "LFE",
            display_chars: &["L", "F", "E"],
        }],
    },
    MeterGroupSpec {
        name: "Side",
        channels: &[
            MeterChannelSpec {
                index: 4,
                label: "SL",
                display_chars: &["S", "L"],
            },
            MeterChannelSpec {
                index: 5,
                label: "SR",
                display_chars: &["S", "R"],
            },
        ],
    },
    MeterGroupSpec {
        name: "Rear",
        channels: &[
            MeterChannelSpec {
                index: 6,
                label: "BL",
                display_chars: &["B", "L"],
            },
            MeterChannelSpec {
                index: 7,
                label: "BR",
                display_chars: &["B", "R"],
            },
        ],
    },
];

const METER_GROUPS_5_1_2: &[MeterGroupSpec] = &[
    MeterGroupSpec {
        name: "L/R",
        channels: &[
            MeterChannelSpec {
                index: 0,
                label: "L",
                display_chars: &["L"],
            },
            MeterChannelSpec {
                index: 1,
                label: "R",
                display_chars: &["R"],
            },
        ],
    },
    MeterGroupSpec {
        name: "Center",
        channels: &[MeterChannelSpec {
            index: 2,
            label: "C",
            display_chars: &["C"],
        }],
    },
    MeterGroupSpec {
        name: "LFE",
        channels: &[MeterChannelSpec {
            index: 3,
            label: "LFE",
            display_chars: &["L", "F", "E"],
        }],
    },
    MeterGroupSpec {
        name: "Surrounds",
        channels: &[
            MeterChannelSpec {
                index: 4,
                label: "SL",
                display_chars: &["S", "L"],
            },
            MeterChannelSpec {
                index: 5,
                label: "SR",
                display_chars: &["S", "R"],
            },
        ],
    },
    MeterGroupSpec {
        name: "Height",
        channels: &[
            MeterChannelSpec {
                index: 6,
                label: "TFL",
                display_chars: &["T", "F", "L"],
            },
            MeterChannelSpec {
                index: 7,
                label: "TFR",
                display_chars: &["T", "F", "R"],
            },
        ],
    },
];

const METER_GROUPS_5_1_4: &[MeterGroupSpec] = &[
    MeterGroupSpec {
        name: "L/R",
        channels: &[
            MeterChannelSpec {
                index: 0,
                label: "L",
                display_chars: &["L"],
            },
            MeterChannelSpec {
                index: 1,
                label: "R",
                display_chars: &["R"],
            },
        ],
    },
    MeterGroupSpec {
        name: "Center",
        channels: &[MeterChannelSpec {
            index: 2,
            label: "C",
            display_chars: &["C"],
        }],
    },
    MeterGroupSpec {
        name: "LFE",
        channels: &[MeterChannelSpec {
            index: 3,
            label: "LFE",
            display_chars: &["L", "F", "E"],
        }],
    },
    MeterGroupSpec {
        name: "Surrounds",
        channels: &[
            MeterChannelSpec {
                index: 4,
                label: "SL",
                display_chars: &["S", "L"],
            },
            MeterChannelSpec {
                index: 5,
                label: "SR",
                display_chars: &["S", "R"],
            },
        ],
    },
    MeterGroupSpec {
        name: "Height",
        channels: &[
            MeterChannelSpec {
                index: 6,
                label: "TFL",
                display_chars: &["T", "F", "L"],
            },
            MeterChannelSpec {
                index: 7,
                label: "TFR",
                display_chars: &["T", "F", "R"],
            },
            MeterChannelSpec {
                index: 8,
                label: "TBL",
                display_chars: &["T", "B", "L"],
            },
            MeterChannelSpec {
                index: 9,
                label: "TBR",
                display_chars: &["T", "B", "R"],
            },
        ],
    },
];

const METER_GROUPS_7_1_2: &[MeterGroupSpec] = &[
    MeterGroupSpec {
        name: "L/R",
        channels: &[
            MeterChannelSpec {
                index: 0,
                label: "L",
                display_chars: &["L"],
            },
            MeterChannelSpec {
                index: 1,
                label: "R",
                display_chars: &["R"],
            },
        ],
    },
    MeterGroupSpec {
        name: "Center",
        channels: &[MeterChannelSpec {
            index: 2,
            label: "C",
            display_chars: &["C"],
        }],
    },
    MeterGroupSpec {
        name: "LFE",
        channels: &[MeterChannelSpec {
            index: 3,
            label: "LFE",
            display_chars: &["L", "F", "E"],
        }],
    },
    MeterGroupSpec {
        name: "Side",
        channels: &[
            MeterChannelSpec {
                index: 4,
                label: "SL",
                display_chars: &["S", "L"],
            },
            MeterChannelSpec {
                index: 5,
                label: "SR",
                display_chars: &["S", "R"],
            },
        ],
    },
    MeterGroupSpec {
        name: "Rear",
        channels: &[
            MeterChannelSpec {
                index: 6,
                label: "BL",
                display_chars: &["B", "L"],
            },
            MeterChannelSpec {
                index: 7,
                label: "BR",
                display_chars: &["B", "R"],
            },
        ],
    },
    MeterGroupSpec {
        name: "Height",
        channels: &[
            MeterChannelSpec {
                index: 8,
                label: "TFL",
                display_chars: &["T", "F", "L"],
            },
            MeterChannelSpec {
                index: 9,
                label: "TFR",
                display_chars: &["T", "F", "R"],
            },
        ],
    },
];

const METER_GROUPS_7_1_4: &[MeterGroupSpec] = &[
    MeterGroupSpec {
        name: "L/R",
        channels: &[
            MeterChannelSpec {
                index: 0,
                label: "L",
                display_chars: &["L"],
            },
            MeterChannelSpec {
                index: 1,
                label: "R",
                display_chars: &["R"],
            },
        ],
    },
    MeterGroupSpec {
        name: "Center",
        channels: &[MeterChannelSpec {
            index: 2,
            label: "C",
            display_chars: &["C"],
        }],
    },
    MeterGroupSpec {
        name: "LFE",
        channels: &[MeterChannelSpec {
            index: 3,
            label: "LFE",
            display_chars: &["L", "F", "E"],
        }],
    },
    MeterGroupSpec {
        name: "Side",
        channels: &[
            MeterChannelSpec {
                index: 4,
                label: "SL",
                display_chars: &["S", "L"],
            },
            MeterChannelSpec {
                index: 5,
                label: "SR",
                display_chars: &["S", "R"],
            },
        ],
    },
    MeterGroupSpec {
        name: "Rear",
        channels: &[
            MeterChannelSpec {
                index: 6,
                label: "BL",
                display_chars: &["B", "L"],
            },
            MeterChannelSpec {
                index: 7,
                label: "BR",
                display_chars: &["B", "R"],
            },
        ],
    },
    MeterGroupSpec {
        name: "Height",
        channels: &[
            MeterChannelSpec {
                index: 8,
                label: "TFL",
                display_chars: &["T", "F", "L"],
            },
            MeterChannelSpec {
                index: 9,
                label: "TFR",
                display_chars: &["T", "F", "R"],
            },
            MeterChannelSpec {
                index: 10,
                label: "TBL",
                display_chars: &["T", "B", "L"],
            },
            MeterChannelSpec {
                index: 11,
                label: "TBR",
                display_chars: &["T", "B", "R"],
            },
        ],
    },
];

const METER_GROUPS_9_1_4: &[MeterGroupSpec] = &[
    MeterGroupSpec {
        name: "Front",
        channels: &[
            MeterChannelSpec {
                index: 0,
                label: "L",
                display_chars: &["L"],
            },
            MeterChannelSpec {
                index: 1,
                label: "R",
                display_chars: &["R"],
            },
            MeterChannelSpec {
                index: 2,
                label: "WL",
                display_chars: &["W", "L"],
            },
            MeterChannelSpec {
                index: 3,
                label: "WR",
                display_chars: &["W", "R"],
            },
        ],
    },
    MeterGroupSpec {
        name: "Center",
        channels: &[MeterChannelSpec {
            index: 4,
            label: "C",
            display_chars: &["C"],
        }],
    },
    MeterGroupSpec {
        name: "LFE",
        channels: &[MeterChannelSpec {
            index: 5,
            label: "LFE",
            display_chars: &["L", "F", "E"],
        }],
    },
    MeterGroupSpec {
        name: "Side",
        channels: &[
            MeterChannelSpec {
                index: 6,
                label: "SL",
                display_chars: &["S", "L"],
            },
            MeterChannelSpec {
                index: 7,
                label: "SR",
                display_chars: &["S", "R"],
            },
        ],
    },
    MeterGroupSpec {
        name: "Rear",
        channels: &[
            MeterChannelSpec {
                index: 8,
                label: "BL",
                display_chars: &["B", "L"],
            },
            MeterChannelSpec {
                index: 9,
                label: "BR",
                display_chars: &["B", "R"],
            },
        ],
    },
    MeterGroupSpec {
        name: "Height",
        channels: &[
            MeterChannelSpec {
                index: 10,
                label: "TFL",
                display_chars: &["T", "F", "L"],
            },
            MeterChannelSpec {
                index: 11,
                label: "TFR",
                display_chars: &["T", "F", "R"],
            },
            MeterChannelSpec {
                index: 12,
                label: "TBL",
                display_chars: &["T", "B", "L"],
            },
            MeterChannelSpec {
                index: 13,
                label: "TBR",
                display_chars: &["T", "B", "R"],
            },
        ],
    },
];

const METER_GROUPS_9_1_6: &[MeterGroupSpec] = &[
    MeterGroupSpec {
        name: "Front",
        channels: &[
            MeterChannelSpec {
                index: 0,
                label: "L",
                display_chars: &["L"],
            },
            MeterChannelSpec {
                index: 1,
                label: "R",
                display_chars: &["R"],
            },
            MeterChannelSpec {
                index: 2,
                label: "WL",
                display_chars: &["W", "L"],
            },
            MeterChannelSpec {
                index: 3,
                label: "WR",
                display_chars: &["W", "R"],
            },
        ],
    },
    MeterGroupSpec {
        name: "Center",
        channels: &[MeterChannelSpec {
            index: 4,
            label: "C",
            display_chars: &["C"],
        }],
    },
    MeterGroupSpec {
        name: "LFE",
        channels: &[MeterChannelSpec {
            index: 5,
            label: "LFE",
            display_chars: &["L", "F", "E"],
        }],
    },
    MeterGroupSpec {
        name: "Side",
        channels: &[
            MeterChannelSpec {
                index: 6,
                label: "SL",
                display_chars: &["S", "L"],
            },
            MeterChannelSpec {
                index: 7,
                label: "SR",
                display_chars: &["S", "R"],
            },
        ],
    },
    MeterGroupSpec {
        name: "Rear",
        channels: &[
            MeterChannelSpec {
                index: 8,
                label: "BL",
                display_chars: &["B", "L"],
            },
            MeterChannelSpec {
                index: 9,
                label: "BR",
                display_chars: &["B", "R"],
            },
        ],
    },
    MeterGroupSpec {
        name: "Top Front",
        channels: &[
            MeterChannelSpec {
                index: 10,
                label: "TFL",
                display_chars: &["T", "F", "L"],
            },
            MeterChannelSpec {
                index: 11,
                label: "TFR",
                display_chars: &["T", "F", "R"],
            },
        ],
    },
    MeterGroupSpec {
        name: "Top Mid",
        channels: &[
            MeterChannelSpec {
                index: 12,
                label: "TML",
                display_chars: &["T", "M", "L"],
            },
            MeterChannelSpec {
                index: 13,
                label: "TMR",
                display_chars: &["T", "M", "R"],
            },
        ],
    },
    MeterGroupSpec {
        name: "Top Rear",
        channels: &[
            MeterChannelSpec {
                index: 14,
                label: "TBL",
                display_chars: &["T", "B", "L"],
            },
            MeterChannelSpec {
                index: 15,
                label: "TBR",
                display_chars: &["T", "B", "R"],
            },
        ],
    },
];

// ============================================================================
// Standard Speaker Configurations
// ============================================================================

/// 1.0 Mono
pub const CONFIG_1_0: SpeakerConfig = SpeakerConfig {
    id: "1.0",
    name: "1.0 Mono",
    description: "Single channel mono",
    total_channels: 1,
    speakers: &[SpeakerPosition {
        label: "M",
        name: "Mono",
        azimuth: 0.0,
        elevation: 0.0,
        channel: 0,
        is_lfe: false,
    }],
    meter_groups: METER_GROUPS_1_0,
};

/// 2.0 Stereo
pub const CONFIG_2_0: SpeakerConfig = SpeakerConfig {
    id: "2.0",
    name: "2.0 Stereo",
    description: "Standard stereo (left and right)",
    total_channels: 2,
    speakers: &[
        SpeakerPosition {
            label: "L",
            name: "Left",
            azimuth: 30.0,
            elevation: 0.0,
            channel: 0,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "R",
            name: "Right",
            azimuth: -30.0,
            elevation: 0.0,
            channel: 1,
            is_lfe: false,
        },
    ],
    meter_groups: METER_GROUPS_2_0,
};

/// 2.1 Stereo with LFE
pub const CONFIG_2_1: SpeakerConfig = SpeakerConfig {
    id: "2.1",
    name: "2.1 Stereo",
    description: "Stereo with LFE subwoofer channel",
    total_channels: 3,
    speakers: &[
        SpeakerPosition {
            label: "L",
            name: "Left",
            azimuth: 30.0,
            elevation: 0.0,
            channel: 0,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "R",
            name: "Right",
            azimuth: -30.0,
            elevation: 0.0,
            channel: 1,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "LFE",
            name: "Low Frequency Effects",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 2,
            is_lfe: true,
        },
    ],
    meter_groups: METER_GROUPS_2_1,
};

/// 5.0 Surround (no LFE)
pub const CONFIG_5_0: SpeakerConfig = SpeakerConfig {
    id: "5.0",
    name: "5.0 Surround",
    description: "5.0 surround without LFE channel",
    total_channels: 5,
    speakers: &[
        SpeakerPosition {
            label: "FL",
            name: "Front Left",
            azimuth: 30.0,
            elevation: 0.0,
            channel: 0,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "FR",
            name: "Front Right",
            azimuth: -30.0,
            elevation: 0.0,
            channel: 1,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "C",
            name: "Center",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 2,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "SL",
            name: "Side Left",
            azimuth: 110.0,
            elevation: 0.0,
            channel: 3,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "SR",
            name: "Side Right",
            azimuth: -110.0,
            elevation: 0.0,
            channel: 4,
            is_lfe: false,
        },
    ],
    meter_groups: METER_GROUPS_5_0,
};

/// 5.1 Surround (ITU-R BS.775)
pub const CONFIG_5_1: SpeakerConfig = SpeakerConfig {
    id: "5.1",
    name: "5.1 Surround",
    description: "Standard 5.1 surround sound (ITU-R BS.775)",
    total_channels: 6,
    speakers: &[
        SpeakerPosition {
            label: "FL",
            name: "Front Left",
            azimuth: 30.0,
            elevation: 0.0,
            channel: 0,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "FR",
            name: "Front Right",
            azimuth: -30.0,
            elevation: 0.0,
            channel: 1,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "C",
            name: "Center",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 2,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "LFE",
            name: "Low Frequency Effects",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 3,
            is_lfe: true,
        },
        SpeakerPosition {
            label: "SL",
            name: "Side Left",
            azimuth: 110.0,
            elevation: 0.0,
            channel: 4,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "SR",
            name: "Side Right",
            azimuth: -110.0,
            elevation: 0.0,
            channel: 5,
            is_lfe: false,
        },
    ],
    meter_groups: METER_GROUPS_5_1,
};

/// 7.1 Surround
pub const CONFIG_7_1: SpeakerConfig = SpeakerConfig {
    id: "7.1",
    name: "7.1 Surround",
    description: "7.1 surround with side and back speakers",
    total_channels: 8,
    speakers: &[
        SpeakerPosition {
            label: "FL",
            name: "Front Left",
            azimuth: 30.0,
            elevation: 0.0,
            channel: 0,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "FR",
            name: "Front Right",
            azimuth: -30.0,
            elevation: 0.0,
            channel: 1,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "C",
            name: "Center",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 2,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "LFE",
            name: "Low Frequency Effects",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 3,
            is_lfe: true,
        },
        SpeakerPosition {
            label: "SL",
            name: "Side Left",
            azimuth: 90.0,
            elevation: 0.0,
            channel: 4,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "SR",
            name: "Side Right",
            azimuth: -90.0,
            elevation: 0.0,
            channel: 5,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "BL",
            name: "Back Left",
            azimuth: 150.0,
            elevation: 0.0,
            channel: 6,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "BR",
            name: "Back Right",
            azimuth: -150.0,
            elevation: 0.0,
            channel: 7,
            is_lfe: false,
        },
    ],
    meter_groups: METER_GROUPS_7_1,
};

/// 5.1.2 Atmos
pub const CONFIG_5_1_2: SpeakerConfig = SpeakerConfig {
    id: "5.1.2",
    name: "5.1.2 Atmos",
    description: "5.1 with 2 height speakers",
    total_channels: 8,
    speakers: &[
        SpeakerPosition {
            label: "FL",
            name: "Front Left",
            azimuth: 30.0,
            elevation: 0.0,
            channel: 0,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "FR",
            name: "Front Right",
            azimuth: -30.0,
            elevation: 0.0,
            channel: 1,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "C",
            name: "Center",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 2,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "LFE",
            name: "Low Frequency Effects",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 3,
            is_lfe: true,
        },
        SpeakerPosition {
            label: "SL",
            name: "Side Left",
            azimuth: 110.0,
            elevation: 0.0,
            channel: 4,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "SR",
            name: "Side Right",
            azimuth: -110.0,
            elevation: 0.0,
            channel: 5,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TFL",
            name: "Top Front Left",
            azimuth: 30.0,
            elevation: 45.0,
            channel: 6,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TFR",
            name: "Top Front Right",
            azimuth: -30.0,
            elevation: 45.0,
            channel: 7,
            is_lfe: false,
        },
    ],
    meter_groups: METER_GROUPS_5_1_2,
};

/// 5.1.4 Atmos
pub const CONFIG_5_1_4: SpeakerConfig = SpeakerConfig {
    id: "5.1.4",
    name: "5.1.4 Atmos",
    description: "5.1 with 4 height speakers",
    total_channels: 10,
    speakers: &[
        SpeakerPosition {
            label: "FL",
            name: "Front Left",
            azimuth: 30.0,
            elevation: 0.0,
            channel: 0,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "FR",
            name: "Front Right",
            azimuth: -30.0,
            elevation: 0.0,
            channel: 1,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "C",
            name: "Center",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 2,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "LFE",
            name: "Low Frequency Effects",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 3,
            is_lfe: true,
        },
        SpeakerPosition {
            label: "SL",
            name: "Side Left",
            azimuth: 110.0,
            elevation: 0.0,
            channel: 4,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "SR",
            name: "Side Right",
            azimuth: -110.0,
            elevation: 0.0,
            channel: 5,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TFL",
            name: "Top Front Left",
            azimuth: 30.0,
            elevation: 45.0,
            channel: 6,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TFR",
            name: "Top Front Right",
            azimuth: -30.0,
            elevation: 45.0,
            channel: 7,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TBL",
            name: "Top Back Left",
            azimuth: 150.0,
            elevation: 45.0,
            channel: 8,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TBR",
            name: "Top Back Right",
            azimuth: -150.0,
            elevation: 45.0,
            channel: 9,
            is_lfe: false,
        },
    ],
    meter_groups: METER_GROUPS_5_1_4,
};

/// 7.1.2 Atmos
pub const CONFIG_7_1_2: SpeakerConfig = SpeakerConfig {
    id: "7.1.2",
    name: "7.1.2 Atmos",
    description: "7.1 with 2 height speakers",
    total_channels: 10,
    speakers: &[
        SpeakerPosition {
            label: "FL",
            name: "Front Left",
            azimuth: 30.0,
            elevation: 0.0,
            channel: 0,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "FR",
            name: "Front Right",
            azimuth: -30.0,
            elevation: 0.0,
            channel: 1,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "C",
            name: "Center",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 2,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "LFE",
            name: "Low Frequency Effects",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 3,
            is_lfe: true,
        },
        SpeakerPosition {
            label: "SL",
            name: "Side Left",
            azimuth: 90.0,
            elevation: 0.0,
            channel: 4,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "SR",
            name: "Side Right",
            azimuth: -90.0,
            elevation: 0.0,
            channel: 5,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "BL",
            name: "Back Left",
            azimuth: 150.0,
            elevation: 0.0,
            channel: 6,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "BR",
            name: "Back Right",
            azimuth: -150.0,
            elevation: 0.0,
            channel: 7,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TFL",
            name: "Top Front Left",
            azimuth: 30.0,
            elevation: 45.0,
            channel: 8,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TFR",
            name: "Top Front Right",
            azimuth: -30.0,
            elevation: 45.0,
            channel: 9,
            is_lfe: false,
        },
    ],
    meter_groups: METER_GROUPS_7_1_2,
};

/// 7.1.4 Atmos
pub const CONFIG_7_1_4: SpeakerConfig = SpeakerConfig {
    id: "7.1.4",
    name: "7.1.4 Atmos",
    description: "7.1 with 4 height speakers",
    total_channels: 12,
    speakers: &[
        SpeakerPosition {
            label: "FL",
            name: "Front Left",
            azimuth: 30.0,
            elevation: 0.0,
            channel: 0,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "FR",
            name: "Front Right",
            azimuth: -30.0,
            elevation: 0.0,
            channel: 1,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "C",
            name: "Center",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 2,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "LFE",
            name: "Low Frequency Effects",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 3,
            is_lfe: true,
        },
        SpeakerPosition {
            label: "SL",
            name: "Side Left",
            azimuth: 90.0,
            elevation: 0.0,
            channel: 4,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "SR",
            name: "Side Right",
            azimuth: -90.0,
            elevation: 0.0,
            channel: 5,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "BL",
            name: "Back Left",
            azimuth: 150.0,
            elevation: 0.0,
            channel: 6,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "BR",
            name: "Back Right",
            azimuth: -150.0,
            elevation: 0.0,
            channel: 7,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TFL",
            name: "Top Front Left",
            azimuth: 30.0,
            elevation: 45.0,
            channel: 8,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TFR",
            name: "Top Front Right",
            azimuth: -30.0,
            elevation: 45.0,
            channel: 9,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TBL",
            name: "Top Back Left",
            azimuth: 150.0,
            elevation: 45.0,
            channel: 10,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TBR",
            name: "Top Back Right",
            azimuth: -150.0,
            elevation: 45.0,
            channel: 11,
            is_lfe: false,
        },
    ],
    meter_groups: METER_GROUPS_7_1_4,
};

/// 9.1.4 Atmos
pub const CONFIG_9_1_4: SpeakerConfig = SpeakerConfig {
    id: "9.1.4",
    name: "9.1.4 Atmos",
    description: "9.1 with 4 height speakers (adds wide channels)",
    total_channels: 14,
    speakers: &[
        SpeakerPosition {
            label: "FL",
            name: "Front Left",
            azimuth: 30.0,
            elevation: 0.0,
            channel: 0,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "FR",
            name: "Front Right",
            azimuth: -30.0,
            elevation: 0.0,
            channel: 1,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "C",
            name: "Center",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 2,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "LFE",
            name: "Low Frequency Effects",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 3,
            is_lfe: true,
        },
        SpeakerPosition {
            label: "SL",
            name: "Side Left",
            azimuth: 90.0,
            elevation: 0.0,
            channel: 4,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "SR",
            name: "Side Right",
            azimuth: -90.0,
            elevation: 0.0,
            channel: 5,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "BL",
            name: "Back Left",
            azimuth: 150.0,
            elevation: 0.0,
            channel: 6,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "BR",
            name: "Back Right",
            azimuth: -150.0,
            elevation: 0.0,
            channel: 7,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "WL",
            name: "Wide Left",
            azimuth: 60.0,
            elevation: 0.0,
            channel: 8,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "WR",
            name: "Wide Right",
            azimuth: -60.0,
            elevation: 0.0,
            channel: 9,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TFL",
            name: "Top Front Left",
            azimuth: 30.0,
            elevation: 45.0,
            channel: 10,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TFR",
            name: "Top Front Right",
            azimuth: -30.0,
            elevation: 45.0,
            channel: 11,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TBL",
            name: "Top Back Left",
            azimuth: 150.0,
            elevation: 45.0,
            channel: 12,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TBR",
            name: "Top Back Right",
            azimuth: -150.0,
            elevation: 45.0,
            channel: 13,
            is_lfe: false,
        },
    ],
    meter_groups: METER_GROUPS_9_1_4,
};

/// 9.1.6 Atmos
pub const CONFIG_9_1_6: SpeakerConfig = SpeakerConfig {
    id: "9.1.6",
    name: "9.1.6 Atmos",
    description: "9.1 with 6 height speakers (adds top mid channels)",
    total_channels: 16,
    speakers: &[
        SpeakerPosition {
            label: "FL",
            name: "Front Left",
            azimuth: 30.0,
            elevation: 0.0,
            channel: 0,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "FR",
            name: "Front Right",
            azimuth: -30.0,
            elevation: 0.0,
            channel: 1,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "C",
            name: "Center",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 2,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "LFE",
            name: "Low Frequency Effects",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 3,
            is_lfe: true,
        },
        SpeakerPosition {
            label: "SL",
            name: "Side Left",
            azimuth: 90.0,
            elevation: 0.0,
            channel: 4,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "SR",
            name: "Side Right",
            azimuth: -90.0,
            elevation: 0.0,
            channel: 5,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "BL",
            name: "Back Left",
            azimuth: 150.0,
            elevation: 0.0,
            channel: 6,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "BR",
            name: "Back Right",
            azimuth: -150.0,
            elevation: 0.0,
            channel: 7,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "WL",
            name: "Wide Left",
            azimuth: 60.0,
            elevation: 0.0,
            channel: 8,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "WR",
            name: "Wide Right",
            azimuth: -60.0,
            elevation: 0.0,
            channel: 9,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TFL",
            name: "Top Front Left",
            azimuth: 30.0,
            elevation: 45.0,
            channel: 10,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TFR",
            name: "Top Front Right",
            azimuth: -30.0,
            elevation: 45.0,
            channel: 11,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TBL",
            name: "Top Back Left",
            azimuth: 150.0,
            elevation: 45.0,
            channel: 12,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TBR",
            name: "Top Back Right",
            azimuth: -150.0,
            elevation: 45.0,
            channel: 13,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TMiL",
            name: "Top Middle Left",
            azimuth: 90.0,
            elevation: 45.0,
            channel: 14,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TMiR",
            name: "Top Middle Right",
            azimuth: -90.0,
            elevation: 45.0,
            channel: 15,
            is_lfe: false,
        },
    ],
    meter_groups: METER_GROUPS_9_1_6,
};

// ============================================================================
// Configuration Lookup
// ============================================================================

/// Get speaker configuration by ID
pub fn get_speaker_config(id: &str) -> Option<&'static SpeakerConfig> {
    match id {
        "1.0" => Some(&CONFIG_1_0),
        "2.0" => Some(&CONFIG_2_0),
        "2.1" => Some(&CONFIG_2_1),
        "5.0" => Some(&CONFIG_5_0),
        "5.1" => Some(&CONFIG_5_1),
        "7.1" => Some(&CONFIG_7_1),
        "5.1.2" => Some(&CONFIG_5_1_2),
        "5.1.4" => Some(&CONFIG_5_1_4),
        "7.1.2" => Some(&CONFIG_7_1_2),
        "7.1.4" => Some(&CONFIG_7_1_4),
        "9.1.4" => Some(&CONFIG_9_1_4),
        "9.1.6" => Some(&CONFIG_9_1_6),
        _ => None,
    }
}

/// Get all available configuration IDs
pub fn get_available_configs() -> &'static [&'static str] {
    &[
        "1.0", "2.0", "2.1", "5.0", "5.1", "7.1", "5.1.2", "5.1.4", "7.1.2", "7.1.4", "9.1.4",
        "9.1.6",
    ]
}

/// Get speaker configuration by number of channels
/// Returns the most common configuration for the given channel count
pub fn get_speaker_config_by_channels(num_channels: usize) -> Option<&'static SpeakerConfig> {
    match num_channels {
        1 => Some(&CONFIG_1_0),
        2 => Some(&CONFIG_2_0),
        3 => Some(&CONFIG_2_1),
        5 => Some(&CONFIG_5_0),
        6 => Some(&CONFIG_5_1),
        8 => Some(&CONFIG_7_1),    // Could also be 5.1.2, prefer 7.1
        10 => Some(&CONFIG_5_1_4), // Could also be 7.1.2, prefer 5.1.4
        12 => Some(&CONFIG_7_1_4),
        14 => Some(&CONFIG_9_1_4),
        16 => Some(&CONFIG_9_1_6),
        _ => None,
    }
}

/// Get meter groups for a speaker configuration ID, or generate fallback for unknown configs
pub fn get_meter_groups(config_id: &str) -> Option<&'static [MeterGroupSpec]> {
    get_speaker_config(config_id).map(|c| c.meter_groups)
}

/// Get meter groups by channel count, or None if unknown
pub fn get_meter_groups_by_channels(num_channels: usize) -> Option<&'static [MeterGroupSpec]> {
    get_speaker_config_by_channels(num_channels).map(|c| c.meter_groups)
}

/// Generate a fallback meter channel spec for a given channel index
/// Returns a heap-allocated MeterChannelSpec for runtime use
pub fn make_fallback_channel(index: usize) -> MeterChannelSpec {
    // Use a static string for the label since we can't allocate at compile time
    // The caller should handle display names separately for fallback channels
    static FALLBACK_LABELS: &[&str] = &[
        "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15", "16",
    ];
    static FALLBACK_CHARS: &[&[&str]] = &[
        &["1"],
        &["2"],
        &["3"],
        &["4"],
        &["5"],
        &["6"],
        &["7"],
        &["8"],
        &["9"],
        &["1", "0"],
        &["1", "1"],
        &["1", "2"],
        &["1", "3"],
        &["1", "4"],
        &["1", "5"],
        &["1", "6"],
    ];

    if index < FALLBACK_LABELS.len() {
        MeterChannelSpec {
            index,
            label: FALLBACK_LABELS[index],
            display_chars: FALLBACK_CHARS[index],
        }
    } else {
        // For channels beyond 16, just use the first entry as placeholder
        MeterChannelSpec {
            index,
            label: "?",
            display_chars: &["?"],
        }
    }
}

// ============================================================================
// VBAP (Vector Base Amplitude Panning)
// ============================================================================

/// Calculate panning gain for a speaker based on source position
/// Uses modified Vector Base Amplitude Panning (VBAP) with improved height handling
///
/// # Arguments
/// * `source_azimuth` - Source azimuth in degrees
/// * `source_elevation` - Source elevation in degrees
/// * `speaker_azimuth` - Speaker azimuth in degrees
/// * `speaker_elevation` - Speaker elevation in degrees
///
/// # Returns
/// Gain value (0.0 to 1.0)
pub fn calculate_panning_gain(
    source_azimuth: f32,
    source_elevation: f32,
    speaker_azimuth: f32,
    speaker_elevation: f32,
) -> f32 {
    // Convert to radians
    let src_az = source_azimuth.to_radians();
    let src_el = source_elevation.to_radians();
    let spk_az = speaker_azimuth.to_radians();
    let spk_el = speaker_elevation.to_radians();

    // Convert spherical to Cartesian coordinates
    let src_x = src_el.cos() * src_az.sin();
    let src_y = src_el.cos() * src_az.cos();
    let src_z = src_el.sin();

    let spk_x = spk_el.cos() * spk_az.sin();
    let spk_y = spk_el.cos() * spk_az.cos();
    let spk_z = spk_el.sin();

    // Calculate dot product (cosine of angle between vectors)
    let dot_product = src_x * spk_x + src_y * spk_y + src_z * spk_z;

    // Clamp to [0, 1]
    let cosine_gain = dot_product.max(0.0);

    // Apply modified panning law for more even distribution
    // Use power law with exponent 0.5 (square root) for gentler rolloff
    // This helps height channels receive more signal and reduces "hole in middle" effect
    // Standard VBAP uses linear (power 1.0), but 0.5-0.7 is more perceptually uniform
    let gain = cosine_gain.powf(0.5);

    log::trace!(
        "[VBAP] source=({:>6.1}°, {:>5.1}°) speaker=({:>6.1}°, {:>5.1}°) cosine={:.4} gain={:.4}",
        source_azimuth,
        source_elevation,
        speaker_azimuth,
        speaker_elevation,
        cosine_gain,
        gain
    );

    gain
}

/// Calculate panning gain with rear wrap-around for speakers beyond 90° from source
///
/// When a speaker is more than 90° away from the source position, the standard VBAP
/// algorithm produces zero gain. This function treats such speakers as receiving
/// a "phantom source" from the rear (source position + 180°), with an attenuation
/// factor to maintain front-back separation.
///
/// This mimics how commercial upmixers create an enveloping soundfield by projecting
/// stereo content to rear speakers.
///
/// # Arguments
/// * `source_azimuth` - Source azimuth in degrees
/// * `source_elevation` - Source elevation in degrees
/// * `speaker_azimuth` - Speaker azimuth in degrees
/// * `speaker_elevation` - Speaker elevation in degrees
/// * `wrap_attenuation` - Attenuation factor for wrapped sources (0.0 to 1.0)
///
/// # Returns
/// Gain value (0.0 to 1.0)
pub fn calculate_panning_gain_with_wraparound(
    source_azimuth: f32,
    source_elevation: f32,
    speaker_azimuth: f32,
    speaker_elevation: f32,
    wrap_attenuation: f32,
) -> f32 {
    // Try direct path first
    let direct_gain = calculate_panning_gain(
        source_azimuth,
        source_elevation,
        speaker_azimuth,
        speaker_elevation,
    );

    // If direct gain is significant, use it
    if direct_gain > 0.01 {
        return direct_gain;
    }

    // Calculate wrapped source position (from rear)
    let wrapped_azimuth = if source_azimuth > 0.0 {
        source_azimuth - 180.0
    } else {
        source_azimuth + 180.0
    };

    let wrapped_gain = calculate_panning_gain(
        wrapped_azimuth,
        source_elevation,
        speaker_azimuth,
        speaker_elevation,
    );

    wrapped_gain * wrap_attenuation
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_speaker_config() {
        assert!(get_speaker_config("5.1").is_some());
        assert!(get_speaker_config("7.1.4").is_some());
        assert!(get_speaker_config("invalid").is_none());
    }

    #[test]
    fn test_config_5_1() {
        let config = get_speaker_config("5.1").unwrap();
        assert_eq!(config.total_channels, 6);
        assert_eq!(config.speakers.len(), 6);
        assert_eq!(config.speakers[0].label, "FL");
        assert!(config.speakers[3].is_lfe);
    }

    #[test]
    fn test_config_7_1_4() {
        let config = get_speaker_config("7.1.4").unwrap();
        assert_eq!(config.total_channels, 12);
        assert_eq!(config.speakers.len(), 12);

        // Check height channels
        let height_speakers: Vec<_> = config
            .speakers
            .iter()
            .filter(|s| s.elevation > 0.0)
            .collect();
        assert_eq!(height_speakers.len(), 4);
    }

    #[test]
    fn test_panning_gain_center() {
        // Source at center (0°, 0°) should have max gain at center speaker
        let gain = calculate_panning_gain(0.0, 0.0, 0.0, 0.0);
        assert!((gain - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_panning_gain_opposite() {
        // Source at front (0°) should have zero gain at back (180°)
        let gain = calculate_panning_gain(0.0, 0.0, 180.0, 0.0);
        assert!(gain < 0.01);
    }

    #[test]
    fn test_panning_gain_orthogonal() {
        // Source at front (0°) and side (90°) are perpendicular
        let gain = calculate_panning_gain(0.0, 0.0, 90.0, 0.0);
        assert!(gain < 0.1); // Should be very low since they're perpendicular
    }

    #[test]
    fn test_panning_gain_elevation() {
        // Test elevation panning
        let gain = calculate_panning_gain(0.0, 45.0, 0.0, 45.0);
        assert!((gain - 1.0).abs() < 0.001);

        // Source at ear level (0°) to speaker at 45° elevation
        // cosine_gain = cos(45°) ≈ 0.707, with power 0.5: gain = 0.707^0.5 ≈ 0.841
        let gain = calculate_panning_gain(0.0, 0.0, 0.0, 45.0);
        assert!(
            gain > 0.80 && gain < 0.90,
            "Expected gain ~0.841, got {}",
            gain
        );
    }

    #[test]
    fn test_panning_gain_5_1_4_scenario() {
        // Test realistic 5.1.4 scenario: left source (30°, 0°) to various speakers

        // To FL (30°, 0°) - perfect match
        let gain_fl = calculate_panning_gain(30.0, 0.0, 30.0, 0.0);
        assert!(
            (gain_fl - 1.0).abs() < 0.001,
            "FL should have gain ~1.0, got {}",
            gain_fl
        );

        // To TFL (30°, 45°) - same azimuth, 45° elevation difference
        // cosine_gain = cos(45°) ≈ 0.707, with power 0.5: gain ≈ 0.841
        let gain_tfl = calculate_panning_gain(30.0, 0.0, 30.0, 45.0);
        assert!(
            gain_tfl > 0.80 && gain_tfl < 0.90,
            "TFL should have gain ~0.841, got {}",
            gain_tfl
        );

        // To C (0°, 0°) - 30° azimuth difference
        // cosine_gain = cos(30°) ≈ 0.866, with power 0.5: gain ≈ 0.930
        let gain_c = calculate_panning_gain(30.0, 0.0, 0.0, 0.0);
        assert!(
            gain_c > 0.90 && gain_c < 0.95,
            "C should have gain ~0.930, got {}",
            gain_c
        );

        // TFL should have reasonable gain compared to FL (not too attenuated)
        let ratio = gain_tfl / gain_fl;
        assert!(
            ratio > 0.75,
            "Height speaker should have >75% of floor speaker gain, got {:.1}%",
            ratio * 100.0
        );
    }

    #[test]
    fn test_panning_gain_wraparound_back_left() {
        // BL at 150° should get zero from standard panning (more than 90° from 30°)
        let standard_gain = calculate_panning_gain(30.0, 0.0, 150.0, 0.0);
        assert!(
            standard_gain < 0.01,
            "Standard panning should give ~0 for BL, got {}",
            standard_gain
        );

        // With wraparound, BL should receive signal from wrapped source at -150°
        // Wrapped source at -150° to speaker at 150° = 60° difference
        // Expected: cosine_gain = cos(60°) = 0.5, with power 0.5: gain = 0.707
        // Then multiplied by wrap_attenuation = 0.7: final ~0.495
        let wrapped_gain = calculate_panning_gain_with_wraparound(30.0, 0.0, 150.0, 0.0, 0.7);
        assert!(
            wrapped_gain > 0.4 && wrapped_gain < 0.6,
            "Wraparound should give ~0.495 for BL, got {}",
            wrapped_gain
        );
    }

    #[test]
    fn test_panning_gain_wraparound_back_right() {
        // BR at -150° should get zero from standard panning (more than 90° from -30°)
        let standard_gain = calculate_panning_gain(-30.0, 0.0, -150.0, 0.0);
        assert!(
            standard_gain < 0.01,
            "Standard panning should give ~0 for BR, got {}",
            standard_gain
        );

        // With wraparound, BR should receive signal from wrapped source at 150°
        // Wrapped source at 150° to speaker at -150° = 60° difference
        let wrapped_gain = calculate_panning_gain_with_wraparound(-30.0, 0.0, -150.0, 0.0, 0.7);
        assert!(
            wrapped_gain > 0.4 && wrapped_gain < 0.6,
            "Wraparound should give ~0.495 for BR, got {}",
            wrapped_gain
        );
    }

    #[test]
    fn test_panning_gain_wraparound_front_unchanged() {
        // Front speakers should use standard panning (no wraparound needed)
        let standard_gain = calculate_panning_gain(30.0, 0.0, 30.0, 0.0);
        let wrapped_gain = calculate_panning_gain_with_wraparound(30.0, 0.0, 30.0, 0.0, 0.7);

        // Should be identical for front speakers
        assert!(
            (standard_gain - wrapped_gain).abs() < 0.001,
            "Front speaker gains should match: standard={}, wrapped={}",
            standard_gain,
            wrapped_gain
        );
    }

    #[test]
    fn test_panning_gain_wraparound_7_1_config() {
        // Test all speakers in 7.1 config get non-zero gains
        let config = get_speaker_config("7.1").unwrap();
        const LEFT_AZIMUTH: f32 = 30.0;
        const RIGHT_AZIMUTH: f32 = -30.0;
        const WRAP_ATTENUATION: f32 = 0.7;

        for speaker in config.speakers.iter() {
            if speaker.is_lfe {
                continue; // LFE uses fixed 0.5 gains
            }

            let is_rear = speaker.azimuth.abs() > 90.0;
            let (left_gain, right_gain) = if is_rear {
                (
                    calculate_panning_gain_with_wraparound(
                        LEFT_AZIMUTH,
                        0.0,
                        speaker.azimuth,
                        speaker.elevation,
                        WRAP_ATTENUATION,
                    ),
                    calculate_panning_gain_with_wraparound(
                        RIGHT_AZIMUTH,
                        0.0,
                        speaker.azimuth,
                        speaker.elevation,
                        WRAP_ATTENUATION,
                    ),
                )
            } else {
                (
                    calculate_panning_gain(LEFT_AZIMUTH, 0.0, speaker.azimuth, speaker.elevation),
                    calculate_panning_gain(RIGHT_AZIMUTH, 0.0, speaker.azimuth, speaker.elevation),
                )
            };

            // At least one of left or right should have non-zero gain
            let max_gain = left_gain.max(right_gain);
            assert!(
                max_gain > 0.1,
                "Speaker {} ({}) should have non-zero gain, got L={:.3}, R={:.3}",
                speaker.label,
                speaker.azimuth,
                left_gain,
                right_gain
            );
        }
    }
}
