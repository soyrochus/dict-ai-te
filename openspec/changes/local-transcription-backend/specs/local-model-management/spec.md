## ADDED Requirements

### Requirement: Known models are described by an embedded manifest
The binary SHALL embed a manifest of known models, each entry carrying an id, display name, download URL, byte size, and SHA-256 checksum. The Settings dialog SHALL populate its model selector from this manifest.

#### Scenario: Model choices come from the manifest
- **WHEN** the user opens Settings with the local backend selected
- **THEN** the model selector lists the manifest entries with their display names and sizes

#### Scenario: Manifest entries carry verification data
- **WHEN** a model is downloaded
- **THEN** its expected size and SHA-256 are taken from the manifest rather than from the server response

### Requirement: Model availability is visible before a session starts
The application SHALL show the status of the configured model as ready, downloading with progress, not downloaded, or missing at the configured path, and SHALL offer a download action when applicable.

#### Scenario: Model is present
- **WHEN** the configured model exists and passes its checksum
- **THEN** Settings reports the model as ready and no download action is offered

#### Scenario: Model has never been downloaded
- **WHEN** the configured model is absent from the models directory
- **THEN** Settings reports it as not downloaded and offers a Download action

#### Scenario: Override path is broken
- **WHEN** `local.model_path` names a file that does not exist
- **THEN** Settings reports the model as missing at that path and names the path in the message

### Requirement: Models download in-app with progress
Model download SHALL run in the background with visible progress in bytes and percentage, and SHALL be cancellable. Downloads SHALL write to a temporary `.part` file, verify the SHA-256 against the manifest, and only then atomically rename into place, so an interrupted or corrupt download never leaves a file that could be mistaken for a valid model.

#### Scenario: Successful download
- **WHEN** the user starts a download for a manifest model
- **THEN** progress is displayed as it proceeds, the checksum is verified on completion, the file is renamed into place, and the model becomes ready

#### Scenario: Cancelled download
- **WHEN** the user cancels an in-progress download
- **THEN** the download stops, the partial file is removed, and the model remains reported as not downloaded

#### Scenario: Checksum mismatch
- **WHEN** a completed download does not match the manifest checksum
- **THEN** the partial file is discarded, the model is not installed, and an error explains that verification failed

#### Scenario: Interrupted download leaves no usable file
- **WHEN** the application exits or the network fails during a download
- **THEN** only a `.part` file may remain and the model is still reported as not downloaded

#### Scenario: UI stays responsive during download
- **WHEN** a large model is downloading
- **THEN** the application remains usable and a remote session can still be started

### Requirement: Models are stored under the application home
Downloaded models SHALL be stored under `~/.dictaite/models/<engine>/<model-id>/`, honouring `DICTAITE_HOME` when set.

#### Scenario: Default location
- **WHEN** the Whisper model `base.en` is downloaded with no `DICTAITE_HOME` set
- **THEN** it is stored under `~/.dictaite/models/whisper/base.en/`

#### Scenario: DICTAITE_HOME is honoured
- **WHEN** `DICTAITE_HOME` points at an alternative absolute directory
- **THEN** models are stored under `<DICTAITE_HOME>/models/<engine>/<model-id>/`

### Requirement: Users can supply their own model file
The `local.model_path` setting SHALL accept an absolute path to an existing model file. When set, it SHALL take precedence over `local.model` and bypass the manifest and download flow entirely.

#### Scenario: Override is used
- **WHEN** `local.model_path` points at a valid model file
- **THEN** that file is loaded, the manifest model id is ignored, and no download is offered

#### Scenario: Override is chosen through a file picker
- **WHEN** the user picks a model file from disk in Settings
- **THEN** `local.model_path` is set to the chosen absolute path and the model status refreshes

### Requirement: Model problems are reported before a session starts
A session on the local backend SHALL refuse to start when the configured model is missing, and SHALL report a load failure without starting when the model file is present but cannot be loaded.

#### Scenario: Missing model blocks the session
- **WHEN** the user starts a session with the local backend and no model available
- **THEN** the session does not start, the message names the configured model, and the download action is offered

#### Scenario: Corrupt model reports a load error
- **WHEN** the model file exists but the engine fails to load it
- **THEN** the engine's error message is surfaced, the model is marked not ready, and no session is started
