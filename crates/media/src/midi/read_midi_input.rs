use midir::{MidiInput, Ignore};
use std::io::{stdin, stdout, Write};
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

fn main() {
    match run() {
        Ok(_) => (),
        Err(err) => println!("Error: {}", err)
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut midi_in = MidiInput::new("MIDI Input Monitor")?;
    midi_in.ignore(Ignore::None);
    
    let in_ports = midi_in.ports();
    
    if in_ports.is_empty() {
        return Err("No MIDI input devices found!".into());
    }
    
    println!("\nAvailable MIDI input devices:");
    for (i, port) in in_ports.iter().enumerate() {
        println!("{}: {}", i, midi_in.port_name(port)?);
    }
    
    print!("\nSelect MIDI input device (0-{}): ", in_ports.len() - 1);
    stdout().flush()?;
    
    let mut input = String::new();
    stdin().read_line(&mut input)?;
    let port_index: usize = input.trim().parse()?;
    
    if port_index >= in_ports.len() {
        return Err("Invalid port number!".into());
    }
    
    print!("Show raw hex data? (y/n): ");
    stdout().flush()?;
    
    let mut input = String::new();
    stdin().read_line(&mut input)?;
    let show_raw = input.trim().to_lowercase() == "y";
    
    let show_raw = Arc::new(Mutex::new(show_raw));
    let show_raw_clone = Arc::clone(&show_raw);
    
    // Create the parser registry
    let parser_registry = MessageParserRegistry::new();
    let parser_registry = Arc::new(parser_registry);
    let parser_registry_clone = Arc::clone(&parser_registry);
    
    let in_port = &in_ports[port_index];
    println!("\nOpening connection to: {}", midi_in.port_name(in_port)?);
    println!("Listening for MIDI messages... (Press Enter to quit)\n");
    
    let _conn_in = midi_in.connect(in_port, "midi-input-connection", move |_timestamp, message, _| {
        let show_raw = show_raw_clone.lock().unwrap();
        handle_midi_message(message, *show_raw, &parser_registry_clone);
    }, ())?;
    
    let mut input = String::new();
    stdin().read_line(&mut input)?;
    
    println!("Closing connection...");
    Ok(())
}

// Trait for MIDI message types
trait MidiMessage {
    fn display(&self) -> String;
    fn get_channel(&self) -> Option<u8> {
        None
    }
}

// Trait for message parsers - each parser knows how to parse one message type
trait MessageParser: Send + Sync {
    fn can_parse(&self, data: &[u8]) -> bool;
    fn parse(&self, data: &[u8]) -> Option<Box<dyn MidiMessage>>;
}

// Registry that holds all parsers
struct MessageParserRegistry {
    parsers: Vec<Box<dyn MessageParser>>,
}

impl MessageParserRegistry {
    fn new() -> Self {
        let mut registry = MessageParserRegistry {
            parsers: Vec::new(),
        };
        
        // Register all parsers
        registry.register(Box::new(NoteOffParser));
        registry.register(Box::new(NoteOnParser));
        registry.register(Box::new(PolyphonicAftertouchParser));
        registry.register(Box::new(ControlChangeParser));
        registry.register(Box::new(ProgramChangeParser));
        registry.register(Box::new(ChannelAftertouchParser));
        registry.register(Box::new(PitchBendParser));
        registry.register(Box::new(UnknownMessageParser)); // Fallback
        
        registry
    }
    
    fn register(&mut self, parser: Box<dyn MessageParser>) {
        self.parsers.push(parser);
    }
    
    fn parse(&self, data: &[u8]) -> Option<Box<dyn MidiMessage>> {
        for parser in &self.parsers {
            if parser.can_parse(data) {
                return parser.parse(data);
            }
        }
        None
    }
}

// Message types
struct NoteOff {
    channel: u8,
    note: u8,
    velocity: u8,
}

impl MidiMessage for NoteOff {
    fn display(&self) -> String {
        format!("Note OFF  - Channel: {}, Note: {}, Velocity: {}", 
            self.channel, self.note, self.velocity)
    }
    
    fn get_channel(&self) -> Option<u8> {
        Some(self.channel)
    }
}

struct NoteOn {
    channel: u8,
    note: u8,
    velocity: u8,
}

impl MidiMessage for NoteOn {
    fn display(&self) -> String {
        format!("Note ON   - Channel: {}, Note: {}, Velocity: {}", 
            self.channel, self.note, self.velocity)
    }
    
    fn get_channel(&self) -> Option<u8> {
        Some(self.channel)
    }
}

struct PolyphonicAftertouch {
    channel: u8,
    note: u8,
    pressure: u8,
}

impl MidiMessage for PolyphonicAftertouch {
    fn display(&self) -> String {
        format!("Aftertouch - Channel: {}, Note: {}, Pressure: {}", 
            self.channel, self.note, self.pressure)
    }
    
    fn get_channel(&self) -> Option<u8> {
        Some(self.channel)
    }
}

struct ControlChange {
    channel: u8,
    controller: u8,
    value: u8,
}

impl MidiMessage for ControlChange {
    fn display(&self) -> String {
        format!("Control Change - Channel: {}, Controller: {}, Value: {}", 
            self.channel, self.controller, self.value)
    }
    
    fn get_channel(&self) -> Option<u8> {
        Some(self.channel)
    }
}

struct ProgramChange {
    channel: u8,
    program: u8,
}

impl MidiMessage for ProgramChange {
    fn display(&self) -> String {
        format!("Program Change - Channel: {}, Program: {}", 
            self.channel, self.program)
    }
    
    fn get_channel(&self) -> Option<u8> {
        Some(self.channel)
    }
}

struct ChannelAftertouch {
    channel: u8,
    pressure: u8,
}

impl MidiMessage for ChannelAftertouch {
    fn display(&self) -> String {
        format!("Channel Aftertouch - Channel: {}, Pressure: {}", 
            self.channel, self.pressure)
    }
    
    fn get_channel(&self) -> Option<u8> {
        Some(self.channel)
    }
}

struct PitchBend {
    channel: u8,
    value: u16,
}

impl MidiMessage for PitchBend {
    fn display(&self) -> String {
        format!("Pitch Bend - Channel: {}, Value: {}", 
            self.channel, self.value)
    }
    
    fn get_channel(&self) -> Option<u8> {
        Some(self.channel)
    }
}

struct UnknownMessage {
    raw_data: Vec<u8>,
}

impl MidiMessage for UnknownMessage {
    fn display(&self) -> String {
        let mut output = String::from("Unknown/System message: ");
        for byte in &self.raw_data {
            output.push_str(&format!("{:02X} ", byte));
        }
        output
    }
}

// Parser implementations - each parser is responsible for one message type
struct NoteOffParser;

impl MessageParser for NoteOffParser {
    fn can_parse(&self, data: &[u8]) -> bool {
        !data.is_empty() && (data[0] & 0xF0) == 0x80 && data.len() >= 3
    }
    
    fn parse(&self, data: &[u8]) -> Option<Box<dyn MidiMessage>> {
        let channel = (data[0] & 0x0F) + 1;
        Some(Box::new(NoteOff {
            channel,
            note: data[1],
            velocity: data[2],
        }))
    }
}

struct NoteOnParser;

impl MessageParser for NoteOnParser {
    fn can_parse(&self, data: &[u8]) -> bool {
        !data.is_empty() && (data[0] & 0xF0) == 0x90 && data.len() >= 3
    }
    
    fn parse(&self, data: &[u8]) -> Option<Box<dyn MidiMessage>> {
        let channel = (data[0] & 0x0F) + 1;
        if data[2] == 0 {
            // Note On with velocity 0 is treated as Note Off
            Some(Box::new(NoteOff {
                channel,
                note: data[1],
                velocity: 0,
            }))
        } else {
            Some(Box::new(NoteOn {
                channel,
                note: data[1],
                velocity: data[2],
            }))
        }
    }
}

struct PolyphonicAftertouchParser;

impl MessageParser for PolyphonicAftertouchParser {
    fn can_parse(&self, data: &[u8]) -> bool {
        !data.is_empty() && (data[0] & 0xF0) == 0xA0 && data.len() >= 3
    }
    
    fn parse(&self, data: &[u8]) -> Option<Box<dyn MidiMessage>> {
        let channel = (data[0] & 0x0F) + 1;
        Some(Box::new(PolyphonicAftertouch {
            channel,
            note: data[1],
            pressure: data[2],
        }))
    }
}

struct ControlChangeParser;

impl MessageParser for ControlChangeParser {
    fn can_parse(&self, data: &[u8]) -> bool {
        !data.is_empty() && (data[0] & 0xF0) == 0xB0 && data.len() >= 3
    }
    
    fn parse(&self, data: &[u8]) -> Option<Box<dyn MidiMessage>> {
        let channel = (data[0] & 0x0F) + 1;
        Some(Box::new(ControlChange {
            channel,
            controller: data[1],
            value: data[2],
        }))
    }
}

struct ProgramChangeParser;

impl MessageParser for ProgramChangeParser {
    fn can_parse(&self, data: &[u8]) -> bool {
        !data.is_empty() && (data[0] & 0xF0) == 0xC0 && data.len() >= 2
    }
    
    fn parse(&self, data: &[u8]) -> Option<Box<dyn MidiMessage>> {
        let channel = (data[0] & 0x0F) + 1;
        Some(Box::new(ProgramChange {
            channel,
            program: data[1],
        }))
    }
}

struct ChannelAftertouchParser;

impl MessageParser for ChannelAftertouchParser {
    fn can_parse(&self, data: &[u8]) -> bool {
        !data.is_empty() && (data[0] & 0xF0) == 0xD0 && data.len() >= 2
    }
    
    fn parse(&self, data: &[u8]) -> Option<Box<dyn MidiMessage>> {
        let channel = (data[0] & 0x0F) + 1;
        Some(Box::new(ChannelAftertouch {
            channel,
            pressure: data[1],
        }))
    }
}

struct PitchBendParser;

impl MessageParser for PitchBendParser {
    fn can_parse(&self, data: &[u8]) -> bool {
        !data.is_empty() && (data[0] & 0xF0) == 0xE0 && data.len() >= 3
    }
    
    fn parse(&self, data: &[u8]) -> Option<Box<dyn MidiMessage>> {
        let channel = (data[0] & 0x0F) + 1;
        let value = (data[2] as u16) << 7 | (data[1] as u16);
        Some(Box::new(PitchBend {
            channel,
            value,
        }))
    }
}

struct UnknownMessageParser;

impl MessageParser for UnknownMessageParser {
    fn can_parse(&self, _data: &[u8]) -> bool {
        true // Always can parse as fallback
    }
    
    fn parse(&self, data: &[u8]) -> Option<Box<dyn MidiMessage>> {
        Some(Box::new(UnknownMessage {
            raw_data: data.to_vec(),
        }))
    }
}

fn handle_midi_message(data: &[u8], show_raw: bool, registry: &MessageParserRegistry) {
    if let Some(message) = registry.parse(data) {
        if show_raw {
            print!("[");
            for (i, byte) in data.iter().enumerate() {
                if i > 0 {
                    print!(" ");
                }
                print!("{:02X}", byte);
            }
            print!("] ");
        }
        
        println!("{}", message.display());
    }
}