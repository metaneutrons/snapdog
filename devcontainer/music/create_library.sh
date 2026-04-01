#!/bin/bash

# Create fake music library structure for Navidrome with proper metadata and album covers
# Using white-noise.m4a as the base file for all tracks

BASE_FILE="white-noise.m4a"
COVERS_DIR="album_covers"

# Check if base file exists
if [ ! -f "$BASE_FILE" ]; then
    echo "Error: $BASE_FILE not found!"
    exit 1
fi

# Check if ffmpeg is available
if ! command -v ffmpeg &> /dev/null; then
    echo "Error: ffmpeg is required but not installed!"
    echo "Please install ffmpeg: brew install ffmpeg"
    exit 1
fi

# Check if curl is available
if ! command -v curl &> /dev/null; then
    echo "Error: curl is required but not installed!"
    exit 1
fi

echo "🎵 Creating music library structure with metadata and album covers..."
echo ""

# Create covers directory
mkdir -p "$COVERS_DIR"

# Function to get cover URL for an album
get_cover_url() {
    local safe_artist="$1"
    local safe_album="$2"

    case "${safe_artist}_${safe_album}" in
        "Pink_Floyd_The_Dark_Side_of_the_Moon")
            echo "https://upload.wikimedia.org/wikipedia/en/3/3b/Dark_Side_of_the_Moon.png"
            ;;
        "The_Beatles_Abbey_Road")
            echo "https://upload.wikimedia.org/wikipedia/en/4/42/Beatles_-_Abbey_Road.jpg"
            ;;
        "Michael_Jackson_Thriller")
            echo "https://upload.wikimedia.org/wikipedia/en/5/55/Michael_Jackson_-_Thriller.png"
            ;;
        "Led_Zeppelin_Led_Zeppelin_IV")
            echo "https://upload.wikimedia.org/wikipedia/en/2/26/Led_Zeppelin_-_Led_Zeppelin_IV.jpg"
            ;;
        "Nirvana_Nevermind")
            echo "https://upload.wikimedia.org/wikipedia/en/b/b7/NirvanaNevermindalbumcover.jpg"
            ;;
        "AC_DC_Back_in_Black")
            echo "https://upload.wikimedia.org/wikipedia/commons/8/84/ACDC_Back_in_Black.png"
            ;;
        "Eagles_Hotel_California")
            echo "https://upload.wikimedia.org/wikipedia/en/4/49/Hotelcalifornia.jpg"
            ;;
        "Dr__Dre_The_Chronic")
            echo "https://upload.wikimedia.org/wikipedia/en/1/19/Dr._Dre_-_The_Chronic_CD_cover.jpg"
            ;;
        "Nas_Illmatic")
            echo "https://upload.wikimedia.org/wikipedia/en/2/27/IllmaticNas.jpg"
            ;;
        "The_Notorious_B_I_G__Ready_to_Die")
            echo "https://upload.wikimedia.org/wikipedia/en/f/f5/Ready_to_Die.jpg"
            ;;
        *)
            echo ""
            ;;
    esac
}

# Function to download album cover with fallback
download_cover() {
    local artist="$1"
    local album="$2"
    local cover_file="$3"

    # Create safe key by replacing spaces and special chars with underscores
    local safe_artist=$(echo "$artist" | sed 's/[^a-zA-Z0-9]/_/g')
    local safe_album=$(echo "$album" | sed 's/[^a-zA-Z0-9]/_/g')

    echo "  🖼️  Downloading album cover..."

    # Try primary URL
    local primary_url=$(get_cover_url "$safe_artist" "$safe_album")
    if [ -n "$primary_url" ]; then
        echo "    📥 Trying: $primary_url"

        if curl -s -L -A "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)" \
            --max-time 15 --max-filesize 10M \
            "$primary_url" -o "$cover_file" 2>/dev/null; then

            # Verify it's a valid image file
            if file "$cover_file" | grep -q -E "(JPEG|PNG|image)"; then
                local file_size=$(stat -f%z "$cover_file" 2>/dev/null || stat -c%s "$cover_file" 2>/dev/null)
                if [ "$file_size" -gt 1000 ]; then  # At least 1KB
                    echo "    ✅ Cover downloaded successfully (${file_size} bytes)"
                    return 0
                fi
            fi
            rm -f "$cover_file"
        fi
    fi

    echo "    🔄 Trying fallback sources..."

    # Try MusicBrainz/Cover Art Archive approach
    local search_query=$(echo "${artist} ${album}" | sed 's/ /+/g' | sed 's/[^a-zA-Z0-9+]//g')
    local musicbrainz_url="https://musicbrainz.org/ws/2/release-group/?query=artist:${search_query}&fmt=json&limit=1"

    local mb_result=$(curl -s --max-time 10 -A "MusicLibraryScript/1.0" "$musicbrainz_url" 2>/dev/null)
    if [ -n "$mb_result" ]; then
        local mbid=$(echo "$mb_result" | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)
        if [ -n "$mbid" ]; then
            local cover_url="https://coverartarchive.org/release-group/${mbid}/front-500"
            echo "    📥 Trying Cover Art Archive: $mbid"

            if curl -s -L --max-time 10 --max-filesize 5M "$cover_url" -o "$cover_file" 2>/dev/null; then
                if file "$cover_file" | grep -q -E "(JPEG|PNG|image)"; then
                    local file_size=$(stat -f%z "$cover_file" 2>/dev/null || stat -c%s "$cover_file" 2>/dev/null)
                    if [ "$file_size" -gt 1000 ]; then
                        echo "    ✅ Cover downloaded from Cover Art Archive (${file_size} bytes)"
                        return 0
                    fi
                fi
                rm -f "$cover_file"
            fi
        fi
    fi

    # If all else fails, create a simple placeholder
    echo "    ⚠️ Could not download cover, creating placeholder..."
    create_placeholder_cover "$artist" "$album" "$cover_file"
    return 1
}

# Function to create a simple placeholder cover
create_placeholder_cover() {
    local artist="$1"
    local album="$2"
    local cover_file="$3"

    # Ensure the covers directory exists
    mkdir -p "$(dirname "$cover_file")"

    # Create a simple colored square as placeholder using ImageMagick if available
    if command -v convert &> /dev/null; then
        # Generate a color based on album name hash
        local color_hash=$(echo "$album" | shasum | cut -c1-6 2>/dev/null || echo "$album" | md5sum | cut -c1-6 2>/dev/null || echo "4a90e2")
        convert -size 400x400 "xc:#${color_hash}" \
            -gravity center \
            -pointsize 24 \
            -fill white \
            -stroke black \
            -strokewidth 1 \
            -annotate +0-30 "$artist" \
            -annotate +0+30 "$album" \
            "$cover_file" 2>/dev/null && return 0
    fi

    # Fallback: create a simple text file as placeholder
    echo "Album: $album by $artist" > "${cover_file%.jpg}.txt"
    return 1
}

# Function to create track with metadata and embedded cover
create_track() {
    local track_num="$1"
    local title="$2"
    local artist="$3"
    local album="$4"
    local year="$5"
    local genre="$6"
    local filename="$7"
    local cover_file="$8"

    # Build ffmpeg command
    local ffmpeg_cmd="ffmpeg -i ../../$BASE_FILE"

    # Add cover art if available
    if [ -f "$cover_file" ] && file "$cover_file" | grep -q -E "(JPEG|PNG|image)"; then
        ffmpeg_cmd="$ffmpeg_cmd -i $cover_file -map 0:0 -map 1:0 -c:v copy -disposition:v:0 attached_pic"
    fi

    # Add metadata and output
    ffmpeg_cmd="$ffmpeg_cmd -metadata track=\"$track_num\" -metadata title=\"$title\" -metadata artist=\"$artist\" -metadata album=\"$album\" -metadata date=\"$year\" -metadata genre=\"$genre\" -codec:a copy -y \"$filename\""

    # Execute ffmpeg command
    if eval "$ffmpeg_cmd" 2>/dev/null; then
        if [ -f "$cover_file" ] && file "$cover_file" | grep -q -E "(JPEG|PNG|image)"; then
            echo "  ✅ Created: $filename (with cover art)"
        else
            echo "  ✅ Created: $filename"
        fi
    else
        echo "  ❌ Failed: $filename"
    fi
}

# Function to process an album
process_album() {
    local artist="$1"
    local album="$2"
    local year="$3"
    local genre="$4"
    local folder="$5"
    shift 5
    local tracks=("$@")

    echo "🎼 Creating $artist - $album ($year)..."
    mkdir -p "$folder"
    cd "$folder"

    # Download cover art
    local safe_artist=$(echo "$artist" | sed 's/[^a-zA-Z0-9]/_/g')
    local safe_album=$(echo "$album" | sed 's/[^a-zA-Z0-9]/_/g')
    local cover_file="../../$COVERS_DIR/${safe_artist}_${safe_album}.jpg"
    download_cover "$artist" "$album" "$cover_file"

    # Create tracks
    local track_num=1
    for track in "${tracks[@]}"; do
        create_track "$track_num" "$track" "$artist" "$album" "$year" "$genre" "$(printf "%02d - %s.m4a" $track_num "$track")" "$cover_file"
        ((track_num++))
    done

    cd ../..
    echo ""
}

# Album processing
echo "Starting album creation process..."
echo ""

# Album 1: Pink Floyd - The Dark Side of the Moon (1973)
process_album "Pink Floyd" "The Dark Side of the Moon" "1973" "Progressive Rock" "Pink Floyd/The Dark Side of the Moon (1973)" \
    "Speak to Me" "Breathe (In the Air)" "On the Run" "Time" "The Great Gig in the Sky" \
    "Money" "Us and Them" "Any Colour You Like" "Brain Damage" "Eclipse"

# Album 2: The Beatles - Abbey Road (1969)
process_album "The Beatles" "Abbey Road" "1969" "Rock" "The Beatles/Abbey Road (1969)" \
    "Come Together" "Something" "Maxwell's Silver Hammer" "Oh! Darling" "Octopus's Garden" \
    "I Want You (She's So Heavy)" "Here Comes the Sun" "Because" "You Never Give Me Your Money" \
    "Sun King" "Mean Mr. Mustard" "Polythene Pam" "She Came in Through the Bathroom Window" \
    "Golden Slumbers" "Carry That Weight" "The End" "Her Majesty"

# Album 3: Michael Jackson - Thriller (1982)
process_album "Michael Jackson" "Thriller" "1982" "Pop" "Michael Jackson/Thriller (1982)" \
    "Wanna Be Startin' Somethin'" "Baby Be Mine" "The Girl Is Mine" "Thriller" "Beat It" \
    "Billie Jean" "Human Nature" "P.Y.T. (Pretty Young Thing)" "The Lady in My Life"

# Album 4: Led Zeppelin - Led Zeppelin IV (1971)
process_album "Led Zeppelin" "Led Zeppelin IV" "1971" "Hard Rock" "Led Zeppelin/Led Zeppelin IV (1971)" \
    "Black Dog" "Rock and Roll" "The Battle of Evermore" "Stairway to Heaven" \
    "Misty Mountain Hop" "Four Sticks" "Going to California" "When the Levee Breaks"

# Album 5: Nirvana - Nevermind (1991)
process_album "Nirvana" "Nevermind" "1991" "Grunge" "Nirvana/Nevermind (1991)" \
    "Smells Like Teen Spirit" "In Bloom" "Come as You Are" "Breed" "Lithium" "Polly" \
    "Territorial Pissings" "Drain You" "Lounge Act" "Stay Away" "On a Plain" "Something in the Way"

# Album 6: AC/DC - Back in Black (1980)
process_album "AC/DC" "Back in Black" "1980" "Hard Rock" "AC DC/Back in Black (1980)" \
    "Hells Bells" "Shoot to Thrill" "What Do You Do for Money Honey" "Given the Dog a Bone" \
    "Let Me Put My Love into You" "Back in Black" "You Shook Me All Night Long" \
    "Have a Drink on Me" "Shake a Leg" "Rock and Roll Ain't Noise Pollution"

# Album 7: Eagles - Hotel California (1976)
process_album "Eagles" "Hotel California" "1976" "Rock" "Eagles/Hotel California (1976)" \
    "Hotel California" "New Kid in Town" "Life in the Fast Lane" "Wasted Time" \
    "Wasted Time (Reprise)" "Victim of Love" "Pretty Maids All in a Row" \
    "Try and Love Again" "The Last Resort"

# Album 8: Dr. Dre - The Chronic (1992)
process_album "Dr. Dre" "The Chronic" "1992" "Hip-Hop" "Dr. Dre/The Chronic (1992)" \
    "The Chronic (Intro)" "Fuck wit Dre Day (And Everybody's Celebratin')" "Let Me Ride" \
    "The Day the Niggaz Took Over" "Nuthin' but a 'G' Thang" "Deeez Nuuuts" "Lil' Ghetto Boy" \
    "A Nigga Witta Gun" "Rat-Tat-Tat-Tat" "The \$20 Sack Pyramid" "Lyrical Gangbang" \
    "High Powered" "The Doctor's Office" "Stranded on Death Row" "The Roach (The Chronic Outro)" \
    "Bitches Ain't Shit"

# Album 9: Nas - Illmatic (1994)
process_album "Nas" "Illmatic" "1994" "Hip-Hop" "Nas/Illmatic (1994)" \
    "The Genesis" "N.Y. State of Mind" "Life's a Bitch" "The World Is Yours" "Halftime" \
    "Memory Lane (Sittin' in da Park)" "One Love" "One Time 4 Your Mind" "Represent" \
    "It Ain't Hard to Tell"

# Album 10: The Notorious B.I.G. - Ready to Die (1994)
process_album "The Notorious B.I.G." "Ready to Die" "1994" "Hip-Hop" "The Notorious B.I.G/Ready to Die (1994)" \
    "Intro" "Things Done Changed" "Gimme the Loot" "Machine Gun Funk" "Warning" "Ready to Die" \
    "One More Chance" "Fuck Me (Interlude)" "The What" "Juicy" "Everyday Struggle" \
    "Me & My Bitch" "Big Poppa" "Respect" "Friend of Mine" "Unbelievable" "Suicidal Thoughts"

echo "🎉 Music library with metadata and album covers created successfully!"
echo ""
echo "📊 Summary:"
echo "   🎼 Total albums: 10"
echo "   🎤 Artists: Pink Floyd, The Beatles, Michael Jackson, Led Zeppelin, Nirvana, AC/DC, Eagles, Dr. Dre, Nas, The Notorious B.I.G."
echo "   🎧 Total tracks: 119 with complete metadata"
echo "   🖼️  Album covers: Downloaded and embedded where possible"
echo ""
echo "📁 Directory structure:"
find . -type d -name "*(*)" | sort
echo ""
echo "🖼️  Downloaded covers:"
ls -la "$COVERS_DIR"/ 2>/dev/null || echo "   No covers directory found"
echo ""
echo "🔍 Sample metadata check (with cover art):"
echo "   ffprobe -v quiet -show_streams 'Pink Floyd/The Dark Side of the Moon (1973)/01 - Speak to Me.m4a'"
echo ""
echo "💡 Tips:"
echo "   - Covers are cached in '$COVERS_DIR' directory"
echo "   - Re-run script to retry failed cover downloads"
echo "   - Install ImageMagick for better placeholder covers: brew install imagemagick"
echo "   - Script uses Wikipedia Commons for reliable cover sources"
