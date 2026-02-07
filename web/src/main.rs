use leptos::prelude::*;

fn main() {
    println!("Hello, world!");
    leptos::mount::mount_to_body(|| App)
}

#[component]
fn App() -> impl IntoView {
    let (name, set_name) = signal("Controlled".to_string());
    let email = RwSignal::new("".to_string());
    let favorite_color = RwSignal::new("red".to_string());
    let spam_me = RwSignal::new(true);

    view! {
        <input type="text"
            bind:value=(name, set_name)
        />
        <input type="email"
            bind:value=email
        />
        <label>
            "Please send me lots of spam email."
            <input type="checkbox"
                bind:checked=spam_me
            />
        </label>
        <fieldset>
            <legend>"Favorite color"</legend>
            <label>
                "Red"
                <input
                    type="radio"
                    name="color"
                    value="red"
                    bind:group=favorite_color
                />
            </label>
            <label>
                "Green"
                <input
                    type="radio"
                    name="color"
                    value="green"
                    bind:group=favorite_color
                />
            </label>
            <label>
                "Blue"
                <input
                    type="radio"
                    name="color"
                    value="blue"
                    bind:group=favorite_color
                />
            </label>
        </fieldset>
        <p>"Your favorite color is " {favorite_color} "."</p>
        <p>"Name is: " {name}</p>
        <p>"Email is: " {email}</p>
        <Show when=move || spam_me.get()>
            <p>"You’ll receive cool bonus content!"</p>
        </Show>
    }
}
