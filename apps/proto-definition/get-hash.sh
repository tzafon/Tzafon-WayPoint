#!/bin/bash
cat $(ls proto-definition/*.proto | sort) | sha256sum | awk '{print $1}' | tr -d '\n'