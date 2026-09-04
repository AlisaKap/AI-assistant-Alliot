import alliotIdle from "./assets/avatar/alliot-idle.webm";

import {
  useEffect,
  useState,
} from "react";

import {
  invoke,
} from "@tauri-apps/api/core";

import {
  listen,
} from "@tauri-apps/api/event";

import "./index.css";


type AssistantState =
  | "idle"
  | "wake"
  | "recording"
  | "analyzing"
  | "error";


function App() {

  const [
    assistantState,
    setAssistantState,
  ] = useState<AssistantState>("idle");


  // ============================================================
  // TAURI EVENTS
  // ============================================================

  useEffect(() => {

    let mounted = true;

    let unlistenWake: (() => void) | undefined;
    let unlistenListening: (() => void) | undefined;
    let unlistenIdle: (() => void) | undefined;


    const setupListeners = async () => {

      // ========================================================
      // WAKE WORD
      // ========================================================

      try {

        unlistenWake =
          await listen(
            "wake-word-detected",
            () => {

              if (!mounted) {
                return;
              }

              console.log(
                "[UI EVENT] wake-word-detected RECEIVED"
              );

              setAssistantState(
                "wake"
              );

            }
          );

      } catch (error) {

        console.error(
          "[UI] FAILED wake-word-detected:",
          error
        );

      }


      // ========================================================
      // COMMAND LISTENING
      // ========================================================

      try {

        unlistenListening =
          await listen(
            "voice-listening",
            () => {

              if (!mounted) {
                return;
              }

              console.log(
                "[UI EVENT] voice-listening RECEIVED"
              );

              setAssistantState(
                "recording"
              );

            }
          );

      } catch (error) {

        console.error(
          "[UI] FAILED voice-listening:",
          error
        );

      }


      // ========================================================
      // VOICE IDLE
      // ========================================================

      try {

        unlistenIdle =
          await listen(
            "voice-idle",
            () => {

              if (!mounted) {
                return;
              }

              console.log(
                "[UI EVENT] voice-idle RECEIVED"
              );

              setAssistantState(
                "idle"
              );

            }
          );

      } catch (error) {

        console.error(
          "[UI] FAILED voice-idle:",
          error
        );

      }

    };


    setupListeners();


    return () => {

      mounted = false;

      if (unlistenWake) {
        unlistenWake();
      }

      if (unlistenListening) {
        unlistenListening();
      }

      if (unlistenIdle) {
        unlistenIdle();
      }

    };

  }, []);


  // ============================================================
  // MANUAL VOICE BUTTON
  // ============================================================

  const handleVoiceClick =
    async () => {

      if (
        assistantState === "recording" ||
        assistantState === "analyzing"
      ) {

        return;
      }


      setAssistantState(
        "recording"
      );


      try {

        const result =
          await invoke<string>(
            "voice_start"
          );


        console.log(
          "[UI] voice result:",
          result
        );


        setAssistantState(
          "analyzing"
        );


        window.setTimeout(() => {

          setAssistantState(
            "idle"
          );

        }, 500);

      } catch (error) {

        console.error(
          "[UI] voice_start error:",
          error
        );

        showError();

      }

    };


  // ============================================================
  // ERROR
  // ============================================================

  const showError =
    () => {

      setAssistantState(
        "error"
      );


      window.setTimeout(() => {

        setAssistantState(
          "idle"
        );

      }, 2000);

    };


  // ============================================================
  // STATE TEXT
  // ============================================================

  const getStateText =
    () => {

      switch (assistantState) {

        case "idle":
          return "Готов";

        case "wake":
          return "Аллиот";

        case "recording":
          return "Слушаю";

        case "analyzing":
          return "Распознаю";

        case "error":
          return "Ошибка";

        default:
          return "Готов";
      }

    };


  // ============================================================
  // UI
  // ============================================================

  return (

    <main className={`app app--${assistantState}`}>

      <div className="assistant">

        <button
          className="assistant__button"
          type="button"
          onClick={handleVoiceClick}
          disabled={
            assistantState === "recording" ||
            assistantState === "analyzing"
          }
          aria-label="Начать голосовую команду"
        >
          <video
            className="assistant__avatar"
            src={alliotIdle}
            autoPlay
            loop
            muted
            playsInline
          />
        </button>

        <div
          className="assistant__state"
          aria-live="polite"
        >
          {getStateText()}
        </div>

      </div>

    </main>
  );
}


export default App;