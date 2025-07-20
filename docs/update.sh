#!/bin/sh

rm -rf lb-docs
lockbook sync
lockbook export a4d2f517-902e-47ae-8ea6-bac57f7d6e85 .
cp -r lb-docs/* . 
rm -rf lb-docs
