import { invoke } from "@tauri-apps/api/core";
import { useRef } from "react";
import ReactDOM from "react-dom/client";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <App />,
);


function App() {
    let x = useRef('')
    return (
        <>
            <h2>Insert your music folder path here</h2>
            <form>
                <input type="text" name="libinput" id="libinput" onChange={ (event) => x.current = event.target.value as string } />
                <input type="submit" value="sumbit" onClick={(event) => {
                    event.preventDefault();
                    invoke('create_library', { path: x.current }).then(() => {})
                }} />
            </form>
        </>
    )
}
