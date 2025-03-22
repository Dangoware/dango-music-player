import { invoke } from "@tauri-apps/api/core";
import { ChangeEvent, useEffect, useRef, useState } from "react";
import ReactDOM from "react-dom/client";
import { Config, ConfigConnections } from "../types";
import { TauriEvent } from "@tauri-apps/api/event";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <>
        <App />
    </>,
);

function App() {
    let [config, setConfig] = useState<Config>();
    useEffect(() => {
        invoke('get_config').then((_config) => {
            let config = _config as Config;
            console.log(config);

            setConfig(config);
        });
    }, [])

    const last_fm_login = () => {
        invoke('last_fm_init_auth').then(() => {})
    }
    const save_config = () => {
        invoke('save_config', { config: config }).then(() => {
            // invoke('close_window').then(() => {})
        })
    }

    return (
        <>
            <h1>Config</h1>
            <label>last.fm:</label>
            { config?.connections.last_fm_session ? (" already signed in") : (<button onClick={last_fm_login}>sign into last.fm</button>) }
            <br/>
            <br/>

            <ListenBrainz config={ config } setConfig={ setConfig } />
            <br />
            <br />
            <button onClick={ save_config }>save</button>
        </>
    )
}

interface ListenBrainzProps {
    config: Config | undefined,
    setConfig: React.Dispatch<React.SetStateAction<Config | undefined>>,
}
function ListenBrainz({ config, setConfig }: ListenBrainzProps) {
    const [token, setToken] = useState("");

    useEffect( () => {
        console.log("Token: " + token);

        config? setConfig((prev) => ({...prev!, connections: {...config.connections, listenbrainz_token: token}})) :  {}
    }, [token])

    const updateToken =  (e: ChangeEvent<HTMLInputElement>)=> {
        setToken(e.currentTarget.value);
    }
    return (
        <>
            <label>{ "Listenbrainz Token" }</label>
            <input type="text" value={ config?.connections.listenbrainz_token } onChange={updateToken} />
        </>
    )
}