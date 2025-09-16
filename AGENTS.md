
The audio-to-text model should be gpt-4o-transcribe instead of Whisper. 

The gpt model for translation shoulbe gpt-5-mini-2025-08-07 instead of gpt-3.5-turbo

Guarantee that the outputted text, both in the original as well as translated version is properly formatted. With clear spacing, punctuation, blank lines between paragraphs etc etc

There is a small "settings" menu for the following configuration options:

 - Values for male and female. Choose from a list for both man and female. Add a play button which allows listening to the voice

 - Default values for language list box 
 
- Default values for translate to list boxes.  


 The config optiona should be stored in $HOME/.config/dict-ai-te/dict-ai-te_config.toml . Create the dirs/file if it does not exist. Format Toml 

 Read the settings at program launch. Ignore errors if the config file does not exist. Use default values. 