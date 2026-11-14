if [ -d /opt/homebrew/bin ]; then
  PATH="$PATH:/opt/homebrew/bin"
fi
if [ -x "$HOME/.local/share/revia/node-v26.13.1-darwin-arm64/bin/node" ]; then
  PATH="$HOME/.local/share/revia/node-v26.13.1-darwin-arm64/bin:$PATH"
fi
if [ -x /opt/homebrew/opt/rustup/bin/cargo ]; then
  PATH="/opt/homebrew/opt/rustup/bin:$PATH"
elif [ -x "$HOME/.cargo/bin/cargo" ]; then
  PATH="$HOME/.cargo/bin:$PATH"
fi
export PATH="$HOME/.local/bin:$PATH"
