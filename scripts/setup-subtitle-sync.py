#!/usr/bin/env python3
"""Explicitly install the verified base speech model into the server data directory."""
import argparse, hashlib, os, pathlib, shutil, subprocess, sys, tempfile, urllib.request
URL='https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin'
SHA256='60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe'
def digest(path):
    sha=hashlib.sha256()
    with path.open('rb') as file:
        for chunk in iter(lambda:file.read(1024*1024),b''):sha.update(chunk)
    return sha.hexdigest()
def default_data():
    if os.environ.get('SMART_MEDIA_DATA'):return pathlib.Path(os.environ['SMART_MEDIA_DATA'])
    if sys.platform=='darwin':return pathlib.Path.home()/'Library/Application Support/smart-media-scanner'
    if os.name=='nt':return pathlib.Path(os.environ.get('LOCALAPPDATA',str(pathlib.Path.home()/'AppData/Local')))/'smart-media-scanner'
    return pathlib.Path(os.environ.get('XDG_DATA_HOME',str(pathlib.Path.home()/'.local/share')))/'smart-media-scanner'
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--data-dir',type=pathlib.Path,default=default_data());args=parser.parse_args()
    cli=os.environ.get('SMART_MEDIA_WHISPER') or shutil.which('whisper-cli')
    if not cli or not pathlib.Path(cli).is_file():raise SystemExit('Install whisper-cli first (macOS: brew install whisper-cpp). Linux/Windows: build the CLI from https://github.com/ggml-org/whisper.cpp or set SMART_MEDIA_WHISPER.')
    target=pathlib.Path(os.environ.get('SMART_MEDIA_WHISPER_MODEL',str(args.data_dir/'models/ggml-base.bin')));target.parent.mkdir(parents=True,exist_ok=True)
    if target.is_file() and digest(target)==SHA256:print(f'Verified speech model already installed: {target}');return
    temp=None
    try:
        with tempfile.NamedTemporaryFile(prefix='whisper-model-',suffix='.part',dir=target.parent,delete=False) as file:temp=pathlib.Path(file.name)
        print('Downloading the multilingual base model (about 142 MB) from the official whisper.cpp model repository…',flush=True)
        curl=shutil.which('curl')
        if curl:subprocess.run([curl,'--fail','--location','--retry','2','--output',str(temp),URL],check=True)
        else:
            with urllib.request.urlopen(URL,timeout=60) as response,temp.open('wb') as output:shutil.copyfileobj(response,output)
        if digest(temp)!=SHA256:raise SystemExit('Model checksum mismatch. The existing model was left unchanged.')
        temp.replace(target);print(f'Installed and SHA-256 verified: {target}')
    finally:
        if temp is not None:temp.unlink(missing_ok=True)
if __name__=='__main__':main()
