//! Translations. Every user-facing string lives here, one row per key with all languages
//! side by side (English, French, Spanish, German, Italian) so they stay in sync.

use std::cell::Cell;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    En = 0,
    Fr = 1,
    Es = 2,
    De = 3,
    It = 4,
}

impl Lang {
    pub const ALL: [Lang; 5] = [Lang::En, Lang::Fr, Lang::Es, Lang::De, Lang::It];

    pub fn native_name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Fr => "Français",
            Lang::Es => "Español",
            Lang::De => "Deutsch",
            Lang::It => "Italiano",
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Fr => "fr",
            Lang::Es => "es",
            Lang::De => "de",
            Lang::It => "it",
        }
    }

    pub fn from_code(code: &str) -> Option<Lang> {
        let c = code.trim().to_lowercase();
        Lang::ALL.into_iter().find(|l| {
            c == l.code()
                || c.starts_with(&format!("{}-", l.code()))
                || c.starts_with(&format!("{}_", l.code()))
        })
    }

    /// First supported language in the user's macOS preference list.
    pub fn system() -> Lang {
        #[cfg(target_os = "macos")]
        {
            let langs = objc2_foundation::NSLocale::preferredLanguages();
            for l in langs.iter() {
                if let Some(lang) = Lang::from_code(&l.to_string()) {
                    return lang;
                }
            }
        }
        std::env::var("LANG")
            .ok()
            .and_then(|l| Lang::from_code(&l))
            .unwrap_or(Lang::En)
    }
}

thread_local! {
    static LANG: Cell<Lang> = const { Cell::new(Lang::En) };
}

pub fn set_lang(lang: Lang) {
    LANG.with(|l| l.set(lang));
}

pub fn lang() -> Lang {
    LANG.with(|l| l.get())
}

/// Translated string for the current language.
pub fn t(key: T) -> &'static str {
    key.get(lang())
}

macro_rules! strings {
    ($($key:ident => [$en:expr, $fr:expr, $es:expr, $de:expr, $it:expr $(,)?]),* $(,)?) => {
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        pub enum T { $($key),* }
        impl T {
            pub fn get(self, lang: Lang) -> &'static str {
                match self { $(T::$key => [$en, $fr, $es, $de, $it][lang as usize]),* }
            }
        }
    };
}

strings! {
    // ---- navigation ----------------------------------------------------------------
    NavOverview => ["Overview", "Aperçu", "Resumen", "Übersicht", "Panoramica"],
    NavNoise => ["Noise control", "Contrôle du bruit", "Control de ruido", "Geräuschkontrolle", "Controllo del rumore"],
    NavSound => ["Sound", "Son", "Sonido", "Klang", "Suono"],
    NavControls => ["Controls", "Commandes", "Controles", "Steuerung", "Controlli"],
    NavFind => ["Find my earbuds", "Localiser", "Buscar auriculares", "Ohrhörer finden", "Trova auricolari"],
    NavFit => ["Ear tip fit test", "Test des embouts", "Prueba de ajuste", "Passformtest", "Test di aderenza"],
    NavSettings => ["Settings", "Réglages", "Ajustes", "Einstellungen", "Impostazioni"],

    // ---- connection ----------------------------------------------------------------
    Connected => ["Connected", "Connecté", "Conectado", "Verbunden", "Connesso"],
    PhaseWaiting => ["Waiting for your earbuds…", "En attente de vos écouteurs…", "Esperando tus auriculares…", "Warte auf deine Ohrhörer…", "In attesa degli auricolari…"],
    PhaseIdentifying => ["Identifying your device…", "Identification de l'appareil…", "Identificando el dispositivo…", "Gerät wird erkannt…", "Identificazione del dispositivo…"],
    PhaseConnecting => ["Connecting…", "Connexion…", "Conectando…", "Verbinden…", "Connessione…"],
    PhaseSetup => ["Setting up your device…", "Configuration de l'appareil…", "Configurando el dispositivo…", "Gerät wird eingerichtet…", "Configurazione del dispositivo…"],
    PhaseRebooting => ["Restarting your earbuds…", "Redémarrage des écouteurs…", "Reiniciando los auriculares…", "Ohrhörer werden neu gestartet…", "Riavvio degli auricolari…"],
    PhaseRetrying => ["Reconnecting…", "Reconnexion…", "Reconectando…", "Verbindung wird wiederhergestellt…", "Riconnessione…"],
    HintBusy => [
        "Another app is using your earbuds' controls: Ear (web) in a browser, or the Nothing X app on your phone. Close it — the connection resumes automatically.",
        "Une autre application utilise les commandes de vos écouteurs : Ear (web) dans un navigateur, ou l'app Nothing X sur votre téléphone. Fermez-la : la connexion reprendra automatiquement.",
        "Otra app está usando los controles de tus auriculares: Ear (web) en un navegador o la app Nothing X de tu teléfono. Ciérrala y la conexión se reanudará sola.",
        "Eine andere App nutzt die Steuerung deiner Ohrhörer: Ear (web) im Browser oder die Nothing X App auf deinem Handy. Schließe sie – die Verbindung wird automatisch fortgesetzt.",
        "Un'altra app sta usando i controlli degli auricolari: Ear (web) in un browser o l'app Nothing X sul telefono. Chiudila e la connessione riprenderà da sola.",
    ],
    HintWaiting => [
        "Open the case or put your earbuds in. They must be connected to this Mac as an audio device.",
        "Ouvrez le boîtier ou mettez vos écouteurs. Ils doivent être connectés à ce Mac comme appareil audio.",
        "Abre el estuche o ponte los auriculares. Deben estar conectados a este Mac como dispositivo de audio.",
        "Öffne das Case oder setze die Ohrhörer ein. Sie müssen als Audiogerät mit diesem Mac verbunden sein.",
        "Apri la custodia o indossa gli auricolari. Devono essere collegati a questo Mac come dispositivo audio.",
    ],
    Tagline => ["Companion app for Nothing & CMF earbuds", "L'application compagnon de vos écouteurs Nothing & CMF", "La app complementaria para auriculares Nothing y CMF", "Die Begleit-App für Nothing- & CMF-Ohrhörer", "L'app companion per auricolari Nothing e CMF"],
    Unofficial => ["Unofficial", "Non officielle", "No oficial", "Inoffiziell", "Non ufficiale"],
    Connect => ["Connect", "Connecter", "Conectar", "Verbinden", "Connetti"],
    Cancel => ["Cancel", "Annuler", "Cancelar", "Abbrechen", "Annulla"],
    ShowAll => ["Show all paired devices", "Afficher tous les appareils jumelés", "Mostrar todos los dispositivos", "Alle gekoppelten Geräte zeigen", "Mostra tutti i dispositivi"],
    ShowNothingOnly => ["Show Nothing & CMF only", "Afficher seulement Nothing & CMF", "Mostrar solo Nothing y CMF", "Nur Nothing & CMF zeigen", "Mostra solo Nothing e CMF"],
    ConnectedToMac => ["Connected to this Mac", "Connecté à ce Mac", "Conectado a este Mac", "Mit diesem Mac verbunden", "Connesso a questo Mac"],
    NotConnected => ["Not connected", "Non connecté", "No conectado", "Nicht verbunden", "Non connesso"],
    NoDevicesTitle => ["No earbuds paired yet", "Aucun écouteur jumelé", "Aún no hay auriculares", "Noch keine Ohrhörer gekoppelt", "Nessun auricolare associato"],
    NoDevicesBody => [
        "Open the case, hold its button until the light flashes, then pair the earbuds in System Settings > Bluetooth.",
        "Ouvrez le boîtier, maintenez son bouton jusqu'à ce que le voyant clignote, puis jumelez les écouteurs dans Réglages Système > Bluetooth.",
        "Abre el estuche, mantén pulsado su botón hasta que parpadee la luz y empareja los auriculares en Ajustes del Sistema > Bluetooth.",
        "Öffne das Case, halte die Taste, bis das Licht blinkt, und kopple die Ohrhörer in Systemeinstellungen > Bluetooth.",
        "Apri la custodia, tieni premuto il tasto finché la luce lampeggia, poi associa gli auricolari in Impostazioni di Sistema > Bluetooth.",
    ],
    OpenBtSettings => ["Open Bluetooth settings", "Ouvrir les réglages Bluetooth", "Abrir ajustes de Bluetooth", "Bluetooth-Einstellungen öffnen", "Apri impostazioni Bluetooth"],

    // ---- overview / batteries -----------------------------------------------------------
    Left => ["Left", "Gauche", "Izquierdo", "Links", "Sinistro"],
    Right => ["Right", "Droit", "Derecho", "Rechts", "Destro"],
    Case => ["Case", "Boîtier", "Estuche", "Case", "Custodia"],
    On => ["On", "Activé", "Activado", "An", "Attivo"],
    MenuOpen => ["Open Nothing X App", "Ouvrir Nothing X App", "Abrir Nothing X App", "Nothing X App öffnen", "Apri Nothing X App"],
    MenuQuit => ["Quit Nothing X App", "Quitter Nothing X App", "Salir de Nothing X App", "Nothing X App beenden", "Esci da Nothing X App"],
    MenuNoDevice => ["No earbuds connected", "Aucun écouteur connecté", "Ningún auricular conectado", "Keine Ohrhörer verbunden", "Nessun auricolare connesso"],
    Off => ["Off", "Désactivé", "Desactivado", "Aus", "Disattivato"],
    TapToRing => ["Play a sound", "Faire sonner", "Reproducir sonido", "Ton abspielen", "Riproduci suono"],

    // ---- noise control -------------------------------------------------------------
    NoiseSubtitle => ["Choose how much of the world you hear.", "Choisissez ce que vous entendez du monde extérieur.", "Elige cuánto oyes de tu entorno.", "Wähle, wie viel du von deiner Umgebung hörst.", "Scegli quanto sentire del mondo esterno."],
    AncNc => ["Noise cancellation", "Réduction de bruit", "Cancelación de ruido", "Geräuschunterdrückung", "Cancellazione rumore"],
    AncTransparency => ["Transparency", "Transparence", "Transparencia", "Transparenz", "Trasparenza"],
    AncOffDesc => ["No processing, the most natural sound.", "Aucun traitement, le son le plus naturel.", "Sin procesamiento, el sonido más natural.", "Keine Verarbeitung, der natürlichste Klang.", "Nessuna elaborazione, il suono più naturale."],
    AncNcDesc => ["Blocks out the noise around you.", "Isole des bruits qui vous entourent.", "Bloquea el ruido a tu alrededor.", "Blendet die Geräusche um dich herum aus.", "Elimina i rumori intorno a te."],
    AncTranspDesc => ["Lets ambient sound in, so you stay aware.", "Laisse passer les sons ambiants pour rester attentif.", "Deja pasar el sonido ambiente para que estés atento.", "Lässt Umgebungsgeräusche durch, damit du aufmerksam bleibst.", "Lascia passare i suoni ambientali per restare attento."],
    Intensity => ["Intensity", "Intensité", "Intensidad", "Intensität", "Intensità"],
    High => ["High", "Élevée", "Alta", "Hoch", "Alta"],
    Mid => ["Medium", "Moyenne", "Media", "Mittel", "Media"],
    Low => ["Low", "Faible", "Baja", "Niedrig", "Bassa"],
    Adaptive => ["Adaptive", "Adaptative", "Adaptativa", "Adaptiv", "Adattiva"],
    HighDesc => ["Maximum reduction for loud places like planes and trains.", "Réduction maximale pour les lieux bruyants (avion, train).", "Máxima reducción para lugares ruidosos como aviones y trenes.", "Maximale Reduktion für laute Orte wie Flugzeug und Zug.", "Riduzione massima per luoghi rumorosi come aerei e treni."],
    MidDesc => ["Balanced reduction for everyday use.", "Réduction équilibrée pour le quotidien.", "Reducción equilibrada para el día a día.", "Ausgewogene Reduktion für den Alltag.", "Riduzione bilanciata per l'uso quotidiano."],
    LowDesc => ["Light reduction for quieter places.", "Réduction légère pour les endroits calmes.", "Reducción ligera para lugares tranquilos.", "Leichte Reduktion für ruhigere Orte.", "Riduzione leggera per luoghi tranquilli."],
    AdaptiveDesc => ["Adjusts automatically to your surroundings.", "S'ajuste automatiquement à votre environnement.", "Se ajusta automáticamente a tu entorno.", "Passt sich automatisch deiner Umgebung an.", "Si regola automaticamente in base all'ambiente."],
    NoiseTip => [
        "Tip: tap & hold an earbud to switch modes. Choose which ones in Controls.",
        "Astuce : un appui long sur un écouteur change de mode. Choisissez lesquels dans Commandes.",
        "Consejo: mantén pulsado un auricular para cambiar de modo. Elige cuáles en Controles.",
        "Tipp: Tippe & halte einen Ohrhörer, um den Modus zu wechseln. Wähle die Modi unter Steuerung.",
        "Suggerimento: tieni premuto un auricolare per cambiare modalità. Scegli quali in Controlli.",
    ],

    // ---- sound ---------------------------------------------------------------------
    SoundSubtitle => ["Equaliser, bass and spatial audio.", "Égaliseur, basses et audio spatial.", "Ecualizador, graves y audio espacial.", "Equalizer, Bass und 3D-Audio.", "Equalizzatore, bassi e audio spaziale."],
    Equaliser => ["Equaliser", "Égaliseur", "Ecualizador", "Equalizer", "Equalizzatore"],
    DiracDesc => ["Tuned by Dirac for your earbuds.", "Réglé par Dirac pour vos écouteurs.", "Ajustado por Dirac para tus auriculares.", "Von Dirac für deine Ohrhörer abgestimmt.", "Calibrato da Dirac per i tuoi auricolari."],
    Recommended => ["Recommended", "Recommandé", "Recomendado", "Empfohlen", "Consigliato"],
    DiracLdac => ["Not available while LDAC is on.", "Indisponible quand LDAC est activé.", "No disponible con LDAC activado.", "Nicht verfügbar, solange LDAC aktiv ist.", "Non disponibile con LDAC attivo."],
    Pop => ["Pop", "Pop", "Pop", "Pop", "Pop"],
    Rock => ["Rock", "Rock", "Rock", "Rock", "Rock"],
    Classical => ["Classical", "Classique", "Clásica", "Klassik", "Classica"],
    Electronic => ["Electronic", "Électronique", "Electrónica", "Elektronisch", "Elettronica"],
    Vocals => ["Enhance vocals", "Voix renforcées", "Voces realzadas", "Gesang betonen", "Voci in risalto"],
    Custom => ["Custom", "Personnalisé", "Personalizado", "Eigene", "Personalizzato"],
    Bass => ["Bass", "Basses", "Graves", "Bass", "Bassi"],
    MidBand => ["Mid", "Médiums", "Medios", "Mitten", "Medi"],
    Treble => ["Treble", "Aigus", "Agudos", "Höhen", "Alti"],
    Reset => ["Reset", "Réinitialiser", "Restablecer", "Zurücksetzen", "Ripristina"],
    CustomHint => ["Choose Custom to shape your own sound.", "Choisissez Personnalisé pour créer votre propre son.", "Elige Personalizado para crear tu propio sonido.", "Wähle Eigene, um deinen Klang zu gestalten.", "Scegli Personalizzato per creare il tuo suono."],
    DragHint => ["Drag the points to adjust (±6 dB).", "Faites glisser les points pour ajuster (±6 dB).", "Arrastra los puntos para ajustar (±6 dB).", "Ziehe die Punkte zum Anpassen (±6 dB).", "Trascina i punti per regolare (±6 dB)."],
    UltraBass => ["Ultra Bass", "Ultra Bass", "Ultra Bass", "Ultra Bass", "Ultra Bass"],
    UltraBassDesc => ["Deeper, more powerful low end.", "Des basses plus profondes et plus puissantes.", "Graves más profundos y potentes.", "Tiefere, kräftigere Bässe.", "Bassi più profondi e potenti."],
    Level => ["Level", "Niveau", "Nivel", "Stufe", "Livello"],
    SpatialAudio => ["Spatial audio", "Audio spatial", "Audio espacial", "3D-Audio", "Audio spaziale"],
    SpatialDesc => ["Places sound around you, as if you were in the room.", "Place le son autour de vous, comme si vous étiez dans la pièce.", "Sitúa el sonido a tu alrededor, como si estuvieras en la sala.", "Platziert den Klang um dich herum, als wärst du im Raum.", "Posiziona il suono intorno a te, come se fossi nella stanza."],
    SpatialFixed => ["Fixed", "Fixe", "Fijo", "Fest", "Fisso"],
    SpatialHead => ["Head tracking", "Suivi de la tête", "Seguimiento", "Head-Tracking", "Tracciamento"],
    SpatialConcert => ["Concert", "Concert", "Concierto", "Konzert", "Concerto"],
    SpatialTheatre => ["Theatre", "Théâtre", "Teatro", "Theater", "Teatro"],
    SpatialGame => ["Game", "Jeu", "Juego", "Spiel", "Gioco"],

    // ---- controls ------------------------------------------------------------------
    ControlsSubtitle => ["Customise touch gestures and the smart dial.", "Personnalisez les gestes tactiles et la molette.", "Personaliza los gestos táctiles y la rueda.", "Passe Touch-Gesten und das Smart Dial an.", "Personalizza i gesti touch e la rotella."],
    LeftEarbud => ["Left earbud", "Écouteur gauche", "Auricular izquierdo", "Linker Ohrhörer", "Auricolare sinistro"],
    RightEarbud => ["Right earbud", "Écouteur droit", "Auricular derecho", "Rechter Ohrhörer", "Auricolare destro"],
    SmartDial => ["Smart dial", "Molette", "Rueda", "Smart Dial", "Rotella"],
    GDoubleTap => ["Double tap", "Double appui", "Doble toque", "Doppeltippen", "Doppio tocco"],
    GTripleTap => ["Triple tap", "Triple appui", "Triple toque", "Dreifachtippen", "Triplo tocco"],
    GTapHold => ["Tap & hold", "Appui long", "Mantener pulsado", "Tippen & halten", "Tocco prolungato"],
    GDoubleTapHold => ["Double tap & hold", "Double appui long", "Doble toque y mantener", "Doppeltippen & halten", "Doppio tocco prolungato"],
    GSinglePress => ["Single press", "Pression simple", "Pulsación simple", "Einmal drücken", "Pressione singola"],
    GDoublePress => ["Double press", "Double pression", "Doble pulsación", "Zweimal drücken", "Doppia pressione"],
    GTriplePress => ["Triple press", "Triple pression", "Triple pulsación", "Dreimal drücken", "Tripla pressione"],
    GPressHold => ["Press & hold", "Pression longue", "Mantener presionado", "Drücken & halten", "Pressione prolungata"],
    GRotate => ["Rotate", "Rotation", "Girar", "Drehen", "Rotazione"],
    DuringCalls => ["During calls", "Pendant les appels", "Durante llamadas", "Bei Anrufen", "Durante le chiamate"],
    APlayPause => ["Play/Pause", "Lecture/Pause", "Reproducir/Pausa", "Wiedergabe/Pause", "Riproduci/Pausa"],
    ASkipBack => ["Previous track", "Piste précédente", "Pista anterior", "Vorheriger Titel", "Brano precedente"],
    ASkipForward => ["Next track", "Piste suivante", "Pista siguiente", "Nächster Titel", "Brano successivo"],
    AVoice => ["Voice assistant", "Assistant vocal", "Asistente de voz", "Sprachassistent", "Assistente vocale"],
    ANone => ["No action", "Aucune action", "Ninguna acción", "Keine Aktion", "Nessuna azione"],
    AVolUp => ["Volume up", "Volume +", "Subir volumen", "Lauter", "Alza volume"],
    AVolDown => ["Volume down", "Volume −", "Bajar volumen", "Leiser", "Abbassa volume"],
    ALowLag => ["Low lag mode on/off", "Mode faible latence on/off", "Modo baja latencia sí/no", "Low-Lag-Modus an/aus", "Bassa latenza on/off"],
    AAnswer => ["Answer call", "Répondre", "Contestar", "Anruf annehmen", "Rispondi"],
    AAnswerMute => ["Answer / mute mic in calls", "Répondre / couper le micro", "Contestar / silenciar micro", "Annehmen / Mikro stumm", "Rispondi / muta microfono"],
    AHangUp => ["Hang up / decline", "Raccrocher / refuser", "Colgar / rechazar", "Auflegen / ablehnen", "Riaggancia / rifiuta"],
    AVolume => ["Volume control", "Réglage du volume", "Control de volumen", "Lautstärkeregelung", "Controllo volume"],
    FixedTitle => ["Not customisable", "Non personnalisable", "No personalizable", "Nicht anpassbar", "Non personalizzabile"],
    FixedAnswer => ["Double tap answers a call", "Double appui : répondre à un appel", "Doble toque: contestar una llamada", "Doppeltippen nimmt Anrufe an", "Doppio tocco: rispondi a una chiamata"],
    FixedHangUp => ["Triple tap hangs up or declines", "Triple appui : raccrocher ou refuser", "Triple toque: colgar o rechazar", "Dreifachtippen legt auf oder lehnt ab", "Triplo tocco: riaggancia o rifiuta"],
    FixedLock => ["Double press & hold locks or unlocks the dial", "Double pression longue : verrouiller / déverrouiller la molette", "Doble pulsación mantenida: bloquear / desbloquear la rueda", "Zweimal drücken & halten sperrt/entsperrt das Dial", "Doppia pressione prolungata: blocca / sblocca la rotella"],
    CycleTitle => ["Switch between", "Alterner entre", "Alternar entre", "Wechseln zwischen", "Alterna tra"],
    CycleHint => ["Choose at least two. Applies to both earbuds.", "Choisissez-en au moins deux. S'applique aux deux écouteurs.", "Elige al menos dos. Se aplica a ambos auriculares.", "Mindestens zwei wählen. Gilt für beide Ohrhörer.", "Scegline almeno due. Vale per entrambi gli auricolari."],

    // ---- find / fit test -------------------------------------------------------------
    FindSubtitle => ["Play a sound to locate them.", "Faites-les sonner pour les retrouver.", "Reproduce un sonido para encontrarlos.", "Spiele einen Ton ab, um sie zu finden.", "Riproduci un suono per trovarli."],
    FindWarning => ["Take your earbuds out before playing a sound — it can be loud.", "Retirez vos écouteurs avant de les faire sonner : le son peut être fort.", "Quítate los auriculares antes de reproducir el sonido: puede ser fuerte.", "Nimm die Ohrhörer heraus, bevor du den Ton abspielst – er kann laut sein.", "Togli gli auricolari prima di riprodurre il suono: può essere forte."],
    Stop => ["Stop", "Arrêter", "Detener", "Stopp", "Interrompi"],
    Ringing => ["Ringing…", "Sonnerie…", "Sonando…", "Klingelt…", "Squilla…"],
    FitSubtitle => ["Check that your ear tips fit and seal well.", "Vérifiez que vos embouts sont bien ajustés.", "Comprueba que las almohadillas ajustan bien.", "Prüfe, ob die Ohrpassstücke gut sitzen.", "Verifica che i gommini aderiscano bene."],
    FitStep1 => ["Put both earbuds in your ears.", "Placez les deux écouteurs dans vos oreilles.", "Colócate ambos auriculares.", "Setze beide Ohrhörer ein.", "Indossa entrambi gli auricolari."],
    FitStep2 => ["Start the test: a short sound plays.", "Lancez le test : un court son est joué.", "Inicia la prueba: sonará un breve sonido.", "Starte den Test: Ein kurzer Ton wird abgespielt.", "Avvia il test: verrà riprodotto un breve suono."],
    FitStep3 => ["Follow the advice for each side.", "Suivez les conseils pour chaque côté.", "Sigue las indicaciones para cada lado.", "Folge den Hinweisen für jede Seite.", "Segui i consigli per ogni lato."],
    FitStart => ["Start test", "Lancer le test", "Iniciar prueba", "Test starten", "Avvia test"],
    FitAgain => ["Test again", "Refaire le test", "Repetir prueba", "Erneut testen", "Ripeti test"],
    FitRunning => ["Keep still and stay quiet…", "Ne bougez pas et restez silencieux…", "Quédate quieto y en silencio…", "Halte still und sei leise…", "Resta fermo e in silenzio…"],
    FitGood => ["Good seal", "Bon ajustement", "Buen ajuste", "Guter Sitz", "Buona aderenza"],
    FitAdjust => ["Adjust or change size", "Ajustez ou changez de taille", "Ajusta o cambia de talla", "Anpassen oder Größe wechseln", "Sistema o cambia misura"],
    FitNotDetected => ["Not detected", "Non détecté", "No detectado", "Nicht erkannt", "Non rilevato"],
    FitPerfect => ["Perfect, you're ready!", "Parfait, vous êtes prêt !", "¡Perfecto, todo listo!", "Perfekt, alles bereit!", "Perfetto, sei pronto!"],
    FitAdvice => ["Push the earbud in gently and twist it slightly, or try another tip size, then test again.", "Enfoncez doucement l'écouteur en le tournant légèrement, ou essayez une autre taille d'embout, puis refaites le test.", "Empuja suavemente el auricular girándolo un poco o prueba otra talla, y repite la prueba.", "Drücke den Ohrhörer sanft hinein und drehe ihn leicht oder probiere eine andere Größe, dann teste erneut.", "Spingi delicatamente l'auricolare ruotandolo un po' o prova un'altra misura, poi ripeti il test."],
    FitCheckBoth => ["Make sure both earbuds are connected and in your ears.", "Vérifiez que les deux écouteurs sont connectés et dans vos oreilles.", "Asegúrate de que ambos auriculares estén conectados y puestos.", "Stelle sicher, dass beide Ohrhörer verbunden und eingesetzt sind.", "Assicurati che entrambi gli auricolari siano connessi e indossati."],

    // ---- settings ------------------------------------------------------------------
    SettingsSubtitle => ["Earbuds, connection and app.", "Écouteurs, connexion et application.", "Auriculares, conexión y app.", "Ohrhörer, Verbindung und App.", "Auricolari, connessione e app."],
    SecEarbuds => ["Earbuds", "Écouteurs", "Auriculares", "Ohrhörer", "Auricolari"],
    InEar => ["In-ear detection", "Détection intra-auriculaire", "Detección en el oído", "Trageerkennung", "Rilevamento in-ear"],
    InEarDesc => ["Pauses playback when you take an earbud out.", "Met la lecture en pause quand vous retirez un écouteur.", "Pausa la reproducción al quitarte un auricular.", "Pausiert die Wiedergabe, wenn du einen Ohrhörer herausnimmst.", "Mette in pausa quando togli un auricolare."],
    LowLag => ["Low lag mode", "Mode faible latence", "Modo baja latencia", "Low-Lag-Modus", "Bassa latenza"],
    LowLagDesc => ["Reduces latency for games and videos.", "Réduit la latence pour les jeux et les vidéos.", "Reduce la latencia en juegos y vídeos.", "Verringert die Latenz bei Spielen und Videos.", "Riduce la latenza per giochi e video."],
    SecConnection => ["Connection", "Connexion", "Conexión", "Verbindung", "Connessione"],
    DualConn => ["Dual connection", "Double connexion", "Conexión dual", "Dual-Verbindung", "Doppia connessione"],
    DualDesc => ["Stay connected to two devices at once.", "Restez connecté à deux appareils à la fois.", "Mantén dos dispositivos conectados a la vez.", "Mit zwei Geräten gleichzeitig verbunden bleiben.", "Resta connesso a due dispositivi insieme."],
    DevicesConnected => ["{n} of 2 devices connected", "{n} appareil(s) connecté(s) sur 2", "{n} de 2 dispositivos conectados", "{n} von 2 Geräten verbunden", "{n} di 2 dispositivi connessi"],
    ThisMac => ["This Mac", "Ce Mac", "Este Mac", "Dieser Mac", "Questo Mac"],
    UnknownDevice => ["Unknown device", "Appareil inconnu", "Dispositivo desconocido", "Unbekanntes Gerät", "Dispositivo sconosciuto"],
    AudioQuality => ["Audio quality", "Qualité audio", "Calidad de audio", "Audioqualität", "Qualità audio"],
    AudioQualityDesc => ["LDAC streams in higher resolution but uses more battery.", "LDAC offre une meilleure résolution mais consomme plus de batterie.", "LDAC transmite en mayor resolución pero gasta más batería.", "LDAC überträgt in höherer Auflösung, braucht aber mehr Akku.", "LDAC trasmette in alta risoluzione ma consuma più batteria."],
    RestartNote => ["The earbuds restart to apply this change.", "Les écouteurs redémarrent pour appliquer ce changement.", "Los auriculares se reinician para aplicar el cambio.", "Die Ohrhörer starten neu, um dies zu übernehmen.", "Gli auricolari si riavviano per applicare la modifica."],
    SecApp => ["App", "Application", "Aplicación", "App", "App"],
    Language => ["Language", "Langue", "Idioma", "Sprache", "Lingua"],
    System => ["System", "Système", "Sistema", "System", "Sistema"],
    Appearance => ["Appearance", "Apparence", "Apariencia", "Darstellung", "Aspetto"],
    Dark => ["Dark", "Sombre", "Oscuro", "Dunkel", "Scuro"],
    Light => ["Light", "Clair", "Claro", "Hell", "Chiaro"],
    SecAbout => ["About", "À propos", "Acerca de", "Info", "Informazioni"],
    DeviceName => ["Name", "Nom", "Nombre", "Name", "Nome"],
    Model => ["Model", "Modèle", "Modelo", "Modell", "Modello"],
    Firmware => ["Firmware", "Firmware", "Firmware", "Firmware", "Firmware"],
    Serial => ["Serial number", "Numéro de série", "Número de serie", "Seriennummer", "Numero di serie"],
    Address => ["Bluetooth address", "Adresse Bluetooth", "Dirección Bluetooth", "Bluetooth-Adresse", "Indirizzo Bluetooth"],
    Refresh => ["Refresh", "Actualiser", "Actualizar", "Aktualisieren", "Aggiorna"],
    ViewLogs => ["View logs", "Voir le journal", "Ver registro", "Protokoll anzeigen", "Mostra log"],
    Disconnect => ["Disconnect", "Déconnecter", "Desconectar", "Trennen", "Disconnetti"],
    Disclaimer => [
        "Unofficial, non-certified app. Not affiliated with or endorsed by Nothing Technology Limited.",
        "Application non officielle et non homologuée, ni affiliée ni approuvée par Nothing Technology Limited.",
        "App no oficial y no homologada, sin afiliación ni aprobación de Nothing Technology Limited.",
        "Inoffizielle, nicht zertifizierte App – weder mit Nothing Technology Limited verbunden noch von ihr genehmigt.",
        "App non ufficiale e non omologata, né affiliata né approvata da Nothing Technology Limited.",
    ],
    SecCredits => ["Credits", "Crédits", "Créditos", "Danksagung", "Crediti"],
    CreditFirst => [
        "The first Nothing X app for macOS was created by eldandelion (Daniel), based on Arunavo Ray's work. Thank you!",
        "La première app Nothing X pour macOS a été créée par eldandelion (Daniel), d'après le travail d'Arunavo Ray. Merci !",
        "La primera app Nothing X para macOS la creó eldandelion (Daniel), basándose en el trabajo de Arunavo Ray. ¡Gracias!",
        "Die erste Nothing X App für macOS stammt von eldandelion (Daniel), basierend auf der Arbeit von Arunavo Ray. Danke!",
        "La prima app Nothing X per macOS è stata creata da eldandelion (Daniel), sulla base del lavoro di Arunavo Ray. Grazie!",
    ],
    CreditProtocol => [
        "Bluetooth protocol from the Ear (web) project by radiance-project.",
        "Protocole Bluetooth issu du projet Ear (web) de radiance-project.",
        "Protocolo Bluetooth del proyecto Ear (web) de radiance-project.",
        "Bluetooth-Protokoll aus dem Projekt Ear (web) von radiance-project.",
        "Protocollo Bluetooth dal progetto Ear (web) di radiance-project.",
    ],
    CreditTrademarks => [
        "Nothing and CMF are trademarks of Nothing Technology Limited. Product images © Nothing.",
        "Nothing et CMF sont des marques de Nothing Technology Limited. Images des produits © Nothing.",
        "Nothing y CMF son marcas de Nothing Technology Limited. Imágenes de productos © Nothing.",
        "Nothing und CMF sind Marken der Nothing Technology Limited. Produktbilder © Nothing.",
        "Nothing e CMF sono marchi di Nothing Technology Limited. Immagini dei prodotti © Nothing.",
    ],
    UnsupportedTitle => [
        "Model not fully supported yet",
        "Modèle pas encore entièrement pris en charge",
        "Modelo aún no compatible del todo",
        "Modell noch nicht vollständig unterstützt",
        "Modello non ancora pienamente supportato",
    ],
    UnsupportedBody => [
        "This version is made for CMF Buds Pro 2. Battery and basic settings may work, other features may not.",
        "Cette version est conçue pour les CMF Buds Pro 2. La batterie et les réglages de base peuvent fonctionner, le reste peut-être pas.",
        "Esta versión está hecha para los CMF Buds Pro 2. La batería y los ajustes básicos pueden funcionar; el resto quizá no.",
        "Diese Version ist für die CMF Buds Pro 2 gemacht. Akku und Grundeinstellungen können funktionieren, anderes vielleicht nicht.",
        "Questa versione è pensata per i CMF Buds Pro 2. Batteria e impostazioni di base possono funzionare, il resto forse no.",
    ],

    // ---- dialogs & toasts ---------------------------------------------------------------
    Close => ["Close", "Fermer", "Cerrar", "Schließen", "Chiudi"],
    Done => ["Done", "OK", "Listo", "Fertig", "Fine"],
    Copy => ["Copy", "Copier", "Copiar", "Kopieren", "Copia"],
    RestartTitle => ["Restart required", "Redémarrage nécessaire", "Reinicio necesario", "Neustart erforderlich", "Riavvio necessario"],
    RestartBody => ["Your earbuds will restart to apply this change, then reconnect automatically.", "Vos écouteurs vont redémarrer pour appliquer ce changement, puis se reconnecter automatiquement.", "Tus auriculares se reiniciarán para aplicar el cambio y se reconectarán solos.", "Deine Ohrhörer starten neu, um dies zu übernehmen, und verbinden sich dann automatisch.", "Gli auricolari si riavvieranno per applicare la modifica e si ricollegheranno da soli."],
    LdacWarning => ["LDAC may reduce battery life and can stutter where there's a lot of radio interference.", "LDAC peut réduire l'autonomie et saccader en présence de fortes interférences radio.", "LDAC puede reducir la autonomía y entrecortarse con muchas interferencias.", "LDAC kann die Akkulaufzeit verkürzen und bei starken Funkstörungen stocken.", "LDAC può ridurre l'autonomia e interrompersi in presenza di molte interferenze."],
    Restart => ["Restart", "Redémarrer", "Reiniciar", "Neu starten", "Riavvia"],
    Logs => ["Connection log", "Journal de connexion", "Registro de conexión", "Verbindungsprotokoll", "Registro di connessione"],
    ToastNoAck => ["The earbuds didn't confirm the change.", "Les écouteurs n'ont pas confirmé le changement.", "Los auriculares no confirmaron el cambio.", "Die Ohrhörer haben die Änderung nicht bestätigt.", "Gli auricolari non hanno confermato la modifica."],
    ToastConnected => ["Connected to your earbuds", "Connecté à vos écouteurs", "Conectado a tus auriculares", "Mit deinen Ohrhörern verbunden", "Connesso ai tuoi auricolari"],
    ToastCopied => ["Copied", "Copié", "Copiado", "Kopiert", "Copiato"],

    // ---- remembered case level ------------------------------------------------------
    JustNow => ["just now", "à l'instant", "ahora mismo", "gerade eben", "proprio ora"],
    MinutesAgo => ["{n} min ago", "il y a {n} min", "hace {n} min", "vor {n} Min.", "{n} min fa"],
    HoursAgo => ["{n} h ago", "il y a {n} h", "hace {n} h", "vor {n} Std.", "{n} h fa"],
    DaysAgo => ["{n} d ago", "il y a {n} j", "hace {n} d", "vor {n} T.", "{n} g fa"],
    CaseHint => [
        "The case reports its level only while an earbud is inside.",
        "Le boîtier indique son niveau seulement quand un écouteur est dedans.",
        "El estuche indica su nivel solo con un auricular dentro.",
        "Das Case meldet seinen Stand nur, wenn ein Ohrhörer drin ist.",
        "La custodia indica il livello solo con un auricolare dentro.",
    ],

    // ---- devices -------------------------------------------------------------------
    MyDevices => ["My devices", "Mes appareils", "Mis dispositivos", "Meine Geräte", "I miei dispositivi"],
    InUse => ["In use", "Utilisé", "En uso", "Aktiv", "In uso"],
    Switch => ["Switch", "Basculer", "Cambiar", "Wechseln", "Passa"],
    PairNew => ["Pair new earbuds", "Jumeler de nouveaux écouteurs", "Emparejar auriculares nuevos", "Neue Ohrhörer koppeln", "Associa nuovi auricolari"],
    ChangeDevice => ["Change device", "Changer d'appareil", "Cambiar dispositivo", "Gerät wechseln", "Cambia dispositivo"],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes() {
        assert_eq!(Lang::from_code("fr-FR"), Some(Lang::Fr));
        assert_eq!(Lang::from_code("de_DE.UTF-8"), Some(Lang::De));
        assert_eq!(Lang::from_code("pt-BR"), None);
    }

    #[test]
    fn no_empty_strings() {
        for lang in Lang::ALL {
            assert!(!T::Connected.get(lang).is_empty());
            assert!(T::DevicesConnected.get(lang).contains("{n}"));
        }
    }
}
