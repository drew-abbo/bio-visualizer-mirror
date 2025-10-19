// Import necessary stuff from the eframe crate
use eframe::egui;
use crate::egui::Key;

// Main entry point of the program
fn main() -> Result<(), eframe::Error> {
    // Default window settings
    let options = eframe::NativeOptions::default();

    // Start the egui application!
    eframe::run_native(
        "egui Demo", // Window title
        options, // Window settings
        // This creates our app state. 
        // I need to research more on what this line means
        Box::new(|_cc| Ok(Box::new(MyApp::default()))),
    )
}

// This struct holds our application's data (state)
// '#[derive(Default)]' makes it easy to create a starting instance
#[derive(Default)]
struct MyApp {
    label: String,  // For a text input field
    value: i32,     // For a slider
    text_option: bool,   // Decides what the state of the output of the text
                        // True makes the output in a line
                        // False makse the output in a block format
    additional_text: String
}

// Implement the eframe::App trait
// telling eframe how to run it 
impl eframe::App for MyApp {
    //the update method s called every frame to draw the UI
    // &mut self allows this method to modify myapp's fields
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame){
        // Show a central panel where we'll put our UI
        egui::CentralPanel::default().show(ctx, |ui| {
            // ui is our tool to add widgets

            //add a heading
            ui.heading("App #1 :)");

            // arrange items horizontally
            ui.horizontal(|ui| {
                ui.label("Write something and press enter: ");
                // text input linked to self.label
                // &mut self.label lets the widgets change the label field
                let response = ui.text_edit_singleline(&mut self.additional_text);

                if response.lost_focus() && ctx.input(|i| i.key_pressed(Key::Enter)) {
                    if !self.additional_text.is_empty() {
                        self.label = self.label.clone() + " " +  &self.additional_text;
                        self.additional_text.clear();
                    }
                }
            });

            // add a slider linked to self.value
            ui.add(egui::Slider::new(&mut self.value, 0..=2000).text("value"));

            // add a button
            if ui.button("Increment").clicked() {
                // if clicked, increase self.value
                self.value += 7;
            }

            if ui.button("Delete").clicked() {
                if let Some(last_space_index) = self.label.rfind(' ') {
                    self.label.truncate(last_space_index);
                }
                else {
                    self.label.clear();
                }
            }


            
            ui.horizontal(|ui| {
                if ui.button("List").clicked() {
                    //if clicked chooses list option, and turns off block option
                    self.text_option = true;
                }

                if ui.button("Block").clicked() {
                    //if clicked chooses list option, and turns off block option
                    self.text_option = false;
                }
            });
               



            let mut i: i32 = 0;

            
            // text_option being true makes the output in a line
            // That is why there is a newline in the concatenation line
            if self.text_option {
                let mut string_concat: String = self.label.clone();
                while i < self.value {
                    string_concat = self.label.clone() + "\n " + &string_concat.clone();
                    i = i + 1;

                };
            
                ui.label(string_concat);
            }
            else {
                let mut string_concat: String = self.label.clone();
                while i < self.value {
                    // No newline so that the output is one after another in a block
                    string_concat = self.label.clone() + " " + &string_concat.clone();
                    i = i + 1;

                };
            
                ui.label(string_concat);

            }
            
        });    

    }
}