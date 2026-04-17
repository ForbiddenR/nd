package main

import (
	"fmt"
	"os"

	"github.com/user/nd/internal/compose"
	"github.com/user/nd/internal/tui"
	"golang.org/x/term"
)

const version = "0.1.0"

const helpText = `nd - nerdctl compose manager

Usage:
  nd              Start interactive TUI
  nd <command>    Execute command directly

Commands:
  start           Start services (nerdctl compose up -d)
  down            Stop services (nerdctl compose down)
  enter [service] Enter container shell (nerdctl compose exec -it)
  logs [service]  Follow service logs (nerdctl compose logs -f)
  restart [svc]   Restart service (nerdctl compose restart)

Options:
  -h, --help      Show this help message
  -v, --version   Show version

Examples:
  nd                    # Start TUI
  nd start              # Start all services
  nd enter web          # Enter web service container
  nd logs db            # Follow db service logs
`

func main() {
	// Handle help and version flags
	if len(os.Args) > 1 {
		switch os.Args[1] {
		case "-h", "--help", "help":
			fmt.Print(helpText)
			os.Exit(0)
		case "-v", "--version", "version":
			fmt.Printf("nd version %s\n", version)
			os.Exit(0)
		}
	}

	parser := compose.NewParser()

	// Find compose file
	composeFile, found := parser.FindComposeFile()
	if !found {
		fmt.Fprintln(os.Stderr, "Error: no docker-compose.yaml or compose.yaml found in current directory")
		os.Exit(1)
	}

	// Parse services
	services, err := tui.GetServices(parser, composeFile)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error parsing compose file: %v\n", err)
		os.Exit(1)
	}

	if len(services) == 0 {
		fmt.Fprintln(os.Stderr, "Error: no services found in compose file")
		os.Exit(1)
	}

	// Check command line args for direct execution
	if len(os.Args) > 1 {
		action := tui.GetActionFromString(os.Args[1])
		if action == tui.ActionExit {
			fmt.Fprintf(os.Stderr, "Unknown action: %s\n\n%s", os.Args[1], helpText)
			os.Exit(1)
		}

		// If action requires a service and we have multiple, check for second arg
		switch action {
		case tui.ActionEnter, tui.ActionLogs, tui.ActionRestart:
			if len(services) == 1 {
				if err := tui.RunOnce(action, services[0]); err != nil {
					fmt.Fprintf(os.Stderr, "Error: %v\n", err)
					os.Exit(1)
				}
				os.Exit(0)
			}
			if len(os.Args) < 3 {
				fmt.Fprintf(os.Stderr, "Multiple services found: %v\nPlease specify a service: nd %s <service>\n", services, os.Args[1])
				os.Exit(1)
			}
			if err := tui.RunOnce(action, os.Args[2]); err != nil {
				fmt.Fprintf(os.Stderr, "Error: %v\n", err)
				os.Exit(1)
			}
		default:
			if err := tui.RunOnce(action, ""); err != nil {
				fmt.Fprintf(os.Stderr, "Error: %v\n", err)
				os.Exit(1)
			}
		}
		os.Exit(0)
	}

	if !term.IsTerminal(int(os.Stdin.Fd())) || !term.IsTerminal(int(os.Stdout.Fd())) {
		fmt.Fprintln(os.Stderr, "Error: interactive TUI requires a terminal. Run nd with a command like 'nd start' or use it in a real terminal")
		os.Exit(1)
	}

	selectedAction, selectedService, err := tui.Run(services, composeFile)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}

	if selectedAction != tui.ActionExit {
		if err := tui.RunOnce(selectedAction, selectedService); err != nil {
			fmt.Fprintf(os.Stderr, "Error: %v\n", err)
			os.Exit(1)
		}
	}
}
