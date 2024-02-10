#!/usr/bin/env zsh

cd $1 || exit
python3 -m venv venv
source venv/bin/activate
pip3 list
pip3 install git+https://github.com/matanolabs/pySigma-backend-matano.git