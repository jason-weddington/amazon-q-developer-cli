#!/bin/bash

# Remove old Ollama methods from api_client/mod.rs

FILE="crates/chat-cli/src/api_client/mod.rs"

# Remove get_ollama_tools method (find the method and remove until the matching brace)
sed -i '' '/async fn get_ollama_tools/,/^    }$/d' "$FILE"

# Remove send_message_ollama_internal method
sed -i '' '/async fn send_message_ollama_internal/,/^    }$/d' "$FILE"

# Remove convert_conversation_to_ollama method
sed -i '' '/fn convert_conversation_to_ollama/,/^    }$/d' "$FILE"

# Remove convert_images_to_ollama method
sed -i '' '/fn convert_images_to_ollama/,/^    }$/d' "$FILE"

# Remove test methods
sed -i '' '/async fn test_send_message_ollama_no_client/,/^    }$/d' "$FILE"
sed -i '' '/async fn test_convert_simple_conversation_to_ollama/,/^    }$/d' "$FILE"
sed -i '' '/async fn test_convert_images_to_ollama/,/^    }$/d' "$FILE"

# Remove any remaining ollama_client references
sed -i '' 's/ollama_client: None,//g' "$FILE"
sed -i '' '/ollama_client/d' "$FILE"

echo "Removed old Ollama methods from $FILE"
