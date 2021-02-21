function addzhist --on-variable PWD
    zox --add "$PWD"
end

function z -d "Jump to a recent directory."
    set -l target (zox $argv)
    if test $status -eq 0
        cd $target
    else
        return $stats
    end
end