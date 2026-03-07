#!/bin/bash
# Physical RGB Verification Script
# This script tests RGB commands and asks for user confirmation that LEDs respond

set -e

echo "======================================"
echo "  RGB Physical Verification Test"
echo "======================================"
echo ""
echo "This script will send RGB commands to your device."
echo "Please observe the LEDs and confirm if changes occur."
echo ""
read -r -p "Press ENTER to begin..."

# Function to wait for user confirmation
confirm_visual() {
    local expected="$1"
    echo ""
    read -r -p "Do you see $expected? (y/n): " response
    if [[ "$response" != "y" && "$response" != "Y" ]]; then
        echo "❌ FAILED: Expected to see $expected"
        return 1
    fi
    echo "✅ CONFIRMED: $expected"
    return 0
}

echo ""
echo "================================================"
echo "Test 1: Setting color to RED"
echo "================================================"
./target/release/ssgg rgb color red
confirm_visual "ALL LEDs turn RED"

echo ""
echo "================================================"
echo "Test 2: Setting color to GREEN"
echo "================================================"
./target/release/ssgg rgb color green
confirm_visual "ALL LEDs turn GREEN"

echo ""
echo "================================================"
echo "Test 3: Setting color to BLUE"
echo "================================================"
./target/release/ssgg rgb color blue
confirm_visual "ALL LEDs turn BLUE"

echo ""
echo "================================================"
echo "Test 4: Setting color to PURPLE"
echo "================================================"
./target/release/ssgg rgb color "#ff00ff"
confirm_visual "ALL LEDs turn PURPLE/MAGENTA"

echo ""
echo "================================================"
echo "Test 5: Setting color to WHITE"
echo "================================================"
./target/release/ssgg rgb color white
confirm_visual "ALL LEDs turn WHITE"

echo ""
echo "================================================"
echo "Test 6: Brightness at 25%"
echo "================================================"
./target/release/ssgg rgb brightness 25
confirm_visual "LEDs DIM to about 25% brightness"

echo ""
echo "================================================"
echo "Test 7: Brightness at 100%"
echo "================================================"
./target/release/ssgg rgb brightness 100
confirm_visual "LEDs return to FULL brightness"

echo ""
echo "================================================"
echo "Test 8: Turning LEDs OFF"
echo "================================================"
./target/release/ssgg rgb off
confirm_visual "ALL LEDs turn OFF (black/dark)"

echo ""
echo "================================================"
echo "Test 9: Turning LEDs back ON (white)"
echo "================================================"
./target/release/ssgg rgb color white
confirm_visual "ALL LEDs turn back ON (white)"

echo ""
echo "======================================"
echo "  ✅ All Physical Tests PASSED!"
echo "======================================"
echo ""
echo "RGB functionality is working correctly!"
echo "The LEDs respond to all commands as expected."
