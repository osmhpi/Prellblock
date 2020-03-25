# PrellBlock

Bahndaten verlässlich und schnell gepuffert - **Persistente Redundante Einheit für Langzeit-Logging über Blockchain**

## Kurzbeschreibung

PrellBlock ist eine in Rust geschriebene, leichtgewichtige Logging-Blockchaintechnologie, die insbesondere für die Datensicherung im Bahnbetrieb entwickelt wird.
Durch das Execute-Order-Validate verfahren wird sichergestellt, dass Daten auch bei einem Ausfall von allen bis auf eine Einheit gesichert werden können, während in vollem Betrieb die Daten mit Byzantinischer Fehlertoleranz verteilt gespeichert und validiert werden können.