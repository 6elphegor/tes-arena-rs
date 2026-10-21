//! Shared state of the original program that ported routines run against.

use crate::exe::Image;
use crate::mem::Memory;
use crate::random::ArenaRandom;

pub struct Machine {
    pub mem: Memory,
    pub rng: ArenaRandom,
    /// EMS: the 64 KB banks not mapped into the frame (see
    /// `Machine::ems_map`), created as zeros when first needed. Shared
    /// between copies of the machine until one writes a bank
    /// (`Arc::make_mut`), so a copy costs the memory, not every bank.
    pub ems: Vec<std::sync::Arc<Vec<u8>>>,
    /// The mouse driver's motion counters (mickeys, x and y) since they
    /// were last read (INT 33h function 0Bh; see `weapon`).
    pub mouse_motion: (u16, u16),
    /// The mouse driver's report waiting for the controls' read of it
    /// (2826:0D85, in the frame's physics): the cursor and the buttons a
    /// front end gives with `report_mouse`, which the frame's steps
    /// before that read (the actors') don't yet see.
    pub mouse_report: Option<(u16, u16, u8)>,
    /// The buttons (AFF9) as the driver last gave them, while the arrow
    /// keys' handler (2826:0473) holds AFF9 at 1 in their place: what a
    /// read of the mouse inside it gives back (the way out's travel map,
    /// 26F7:0011, `area_exit`).
    pub arrows_buttons: Option<u8>,
    /// The PC's timer: the BIOS's tick count at 0:046C (18.2 a second
    /// since midnight), which 0000:9D60 reads. The port keeps the image at
    /// segment 0, so it lives here: `Engine::tick` advances it with the
    /// game's ticks, and a front end may set it from the wall clock.
    pub bios_ticks: u32,
    /// The sound driver: playing a sound (its status 2: a new one is
    /// dropped), as the front end says; and the sounds played, for the
    /// front end to take (`sound`).
    pub sound_busy: bool,
    pub sounds: Vec<crate::sound::Played>,
    /// The digital driver replayed (a test's, from the original's driver:
    /// its answers taken in turn, the calls the sounds make into it
    /// kept): None, the front end's (`sound_busy`).
    pub digital: Option<crate::digital::Replay>,
    /// The level's sound files by slot, as its INF's @SOUND section named
    /// them (the files the driver's loader, 23BA:0217, puts in the slots).
    pub sound_names: Vec<Vec<u8>>,
    /// The music driver: the songs asked for, for the front end to take
    /// (`music`), and how the one playing is doing, as the front end says
    /// (AIL's sequence status: 0 stopped, 1 playing, 2 done).
    pub songs: Vec<crate::music::Song>,
    pub music_status: u8,
    /// The music driver replayed (a test's, from the original's driver
    /// loaded in the game's memory: its answers taken in turn, the calls
    /// the music makes into it kept): None, the front end's (`songs`,
    /// `music_status`).
    pub music: Option<crate::musicdrv::Replay>,
    /// AX of the last list shown (1194:5734's kind: its panel, its record
    /// and the lines' colour), as the port's callers load it before their
    /// list hook, for the front end to draw the list as the original does.
    pub list_kind: u16,
    /// The mouse's reads and the keys held at each that the front end
    /// gave its list (3159:06A0's loop), for the next list to run its
    /// loop over (`Machine::window_list`); taken by that list. None: the
    /// list takes the pick it is given as the loop would leave it.
    pub list_reads: Option<Vec<crate::listwindow::ListRead>>,
    /// Why the last None was the original's own doing (it faults or hangs
    /// there), where a port routine knows (`original_fault`); None for a
    /// give-up the port can't say that of (something not ported). For
    /// reports only: nothing in the game reads it.
    pub gave_up: Option<&'static str>,
    /// The row tables' inputs as the last 0000:DA8E made them
    /// (`rows_note`, at each of its callers): the drawing builds its copy's
    /// tables from these, as the original draws with the tables it last
    /// made, whatever has changed since (the INF's ceiling after an
    /// area's entry, the height a climb clamps after making them). None
    /// before any: the drawing's from the state then.
    pub rows_made: Option<[u16; 11]>,
    /// With a list, each drawing's row tables' inputs added to it
    /// (`render::rows_read`, as gen_worldframes.py's `rows` lines have
    /// them), for the tests; a copy of the machine run again (a dialog's
    /// turn) takes it back with the rest.
    pub rows_drawn: Option<Vec<[u16; 11]>>,
    /// The row 0000:4CC0 last cleared under the view (C3A2), for the
    /// engine to clear in the drawing's page too, which is the original's
    /// draw page, before its next drawing (`Engine::frame`).
    pub row_cleared: Option<u16>,
    /// DI as the equipment page's last handler left it, which its button
    /// loop (1194:5CF4) keeps for the next (1F56 counts the quests' lines
    /// into DS:DI): None where the port doesn't know it.
    pub page_di: Option<u16>,
    /// DI as the spellmaker's last step left it (1194:6064's handlers,
    /// which its click loop keeps for the next: the element's list, 6A45,
    /// hands it to 5734, which stores it at A872): None where the port
    /// doesn't know it.
    pub spellmaker_di: Option<u16>,
    /// The mouse's reads and the keys typed before each that the front
    /// end gave its menu (3159:0F2A's loop), for the next menu to run its
    /// loop over (`Machine::menu_answered`); taken by that menu. None: the
    /// menu takes the pick it is given.
    pub menu_reads: Option<Vec<crate::menubox::MenuRead>>,
    /// Whether the next list (3159:06A0) is to keep a copy of the game as
    /// its loop starts (`list_captured`), for a front end to show the
    /// list's loop as it runs over the reads so far (`ListCapture::live`).
    pub list_capture: bool,
    pub list_captured: Option<Box<crate::listwindow::ListCapture>>,
    /// FS as the frame left it: the creature segment ([1042]) once the
    /// frame's speeds (2294:115F, with creatures fighting) loaded it since
    /// the drawing's set-up loaded the walls' ([D806], 0000:D476). Read
    /// where the original reads FS without setting it (292B:086C).
    pub fs_creatures: bool,
}

impl Clone for Machine {
    fn clone(&self) -> Self {
        let Machine { mem, rng, ems, mouse_motion, mouse_report, arrows_buttons, bios_ticks, sound_busy, sounds, digital, sound_names, songs, music_status, music, gave_up, list_kind, list_reads, rows_made, rows_drawn, row_cleared, page_di, spellmaker_di, menu_reads, list_capture, list_captured, fs_creatures } = self;
        Machine {
            mem: mem.clone(),
            rng: rng.clone(),
            ems: ems.clone(),
            mouse_motion: *mouse_motion,
            mouse_report: *mouse_report,
            arrows_buttons: *arrows_buttons,
            bios_ticks: *bios_ticks,
            sound_busy: *sound_busy,
            sounds: sounds.clone(),
            digital: digital.clone(),
            sound_names: sound_names.clone(),
            songs: songs.clone(),
            music_status: *music_status,
            music: music.clone(),
            rows_made: *rows_made,
            rows_drawn: rows_drawn.clone(),
            row_cleared: *row_cleared,
            page_di: *page_di,
            spellmaker_di: *spellmaker_di,
            gave_up: *gave_up,
            list_kind: *list_kind,
            list_reads: list_reads.clone(),
            menu_reads: menu_reads.clone(),
            list_capture: *list_capture,
            list_captured: list_captured.clone(),
            fs_creatures: *fs_creatures,
        }
    }

    /// Copied into this machine's own allocations (the memory and the EMS
    /// banks reused): what a front end copying the machine every turn
    /// wants, a fresh copy's megabytes being faulted in page by page.
    fn clone_from(&mut self, source: &Self) {
        let Machine { mem, rng, ems, mouse_motion, mouse_report, arrows_buttons, bios_ticks, sound_busy, sounds, digital, sound_names, songs, music_status, music, gave_up, list_kind, list_reads, rows_made, rows_drawn, row_cleared, page_di, spellmaker_di, menu_reads, list_capture, list_captured, fs_creatures } = source;
        self.mem.clone_from(mem);
        self.rng.clone_from(rng);
        self.ems.clone_from(ems);
        self.mouse_motion = *mouse_motion;
        self.mouse_report = *mouse_report;
        self.arrows_buttons = *arrows_buttons;
        self.bios_ticks = *bios_ticks;
        self.sound_busy = *sound_busy;
        self.sounds.clone_from(sounds);
        self.digital.clone_from(digital);
        self.sound_names.clone_from(sound_names);
        self.songs.clone_from(songs);
        self.music_status = *music_status;
        self.music.clone_from(music);
        self.gave_up = *gave_up;
        self.list_kind = *list_kind;
        self.list_reads.clone_from(list_reads);
        self.rows_made = *rows_made;
        self.rows_drawn.clone_from(rows_drawn);
        self.row_cleared = *row_cleared;
        self.page_di = *page_di;
        self.spellmaker_di = *spellmaker_di;
        self.menu_reads.clone_from(menu_reads);
        self.list_capture = *list_capture;
        self.list_captured.clone_from(list_captured);
        self.fs_creatures = *fs_creatures;
    }
}

impl Machine {
    pub fn new(image: &Image) -> Self {
        let mem = Memory::new(image);
        let rng = ArenaRandom::from_mem(&mem);
        Self { mem, rng, ems: Vec::new(), mouse_motion: (0, 0), mouse_report: None, arrows_buttons: None, bios_ticks: 0, sound_busy: false, sounds: Vec::new(), digital: None, sound_names: Vec::new(), songs: Vec::new(), music_status: 0, music: None, gave_up: None, list_kind: 0, list_reads: None, rows_made: None, rows_drawn: None, row_cleared: None, page_di: None, spellmaker_di: None, menu_reads: None, list_capture: false, list_captured: None, fs_creatures: false }
    }

    /// None, noting `why`: where the original itself faults or hangs, so
    /// the port gives up as the original can't go on (see `gave_up`).
    pub fn original_fault<T>(&mut self, why: &'static str) -> Option<T> {
        self.gave_up = Some(why);
        crate::giveup::original(why)
    }

    /// Reload the RNG after its DGROUP variables were written directly.
    pub fn load_rng(&mut self) {
        self.rng = ArenaRandom::from_mem(&self.mem);
    }

    /// Write the RNG seed back, so memory matches the original.
    pub fn store_rng(&mut self) {
        self.rng.store(&mut self.mem);
    }

    /// `r`, with the RNG seed put back to `saved` where it is None: for
    /// routines that roll with a seed of their own and put the game's back
    /// at their end, where the port gives up before that end (the original
    /// has no such exit).
    pub(crate) fn seed_back<T>(&mut self, saved: u32, r: Option<T>) -> Option<T> {
        if r.is_none() {
            self.rng.seed = saved;
        }
        r
    }

    /// Whether the hook (far pointer) at DS:`at` is set. The original
    /// tests the segment word (0000:A8EF and its inline copies), which is
    /// never 0 for a routine of its own: the image is relocated when it is
    /// loaded, and the hooks into its segment 0000 are written with a
    /// relocated segment (2294:0006, `mov word [C3AA], 0000` with a
    /// relocation). The port keeps the image at segment 0, so those hooks
    /// have segment 0 here. A hook is cleared with the whole pointer
    /// (0000:0971 `mov dword [CD32], 0` and the like), so the port tests
    /// the whole pointer.
    pub fn hook_set(&self, at: u16) -> bool {
        self.mem.r32(crate::mem::DS, at) != 0
    }
}
