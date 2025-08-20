# Fork Management Workflow

## Daily Development Workflow

### 1. Start New Feature
```bash
# Always start from clean main
git checkout main
git pull upstream main
git push origin main  # Keep your fork's main in sync

# Create feature branch
git checkout -b feature/my-new-feature
```

### 2. Work on Feature
```bash
# Make commits on feature branch
git add .
git commit -m "Add new feature"

# Push feature branch to your fork
git push origin feature/my-new-feature
```

### 3. Keep Feature Branch Updated
```bash
# Periodically sync with upstream
git checkout main
git pull upstream main
git push origin main

# Rebase your feature branch
git checkout feature/my-new-feature
git rebase main
```

### 4. Create Pull Request
- Create PR from `your-fork:feature/my-new-feature` to `aws:main`
- Never create PR from `your-fork:main` to `aws:main`

## Weekly Maintenance

### Sync Fork with Upstream
```bash
# Fetch all updates
git fetch upstream
git fetch origin

# Update main
git checkout main
git merge upstream/main  # or git rebase upstream/main
git push origin main

# Clean up merged branches
git branch -d feature/completed-feature
git push origin --delete feature/completed-feature
```

## Emergency: Fix Diverged Main

If your main has diverged from upstream:

```bash
# Backup your work first!
git checkout main
git branch backup-main

# Reset to upstream
git reset --hard upstream/main
git push origin main --force
```

## Best Practices

1. **Never commit directly to main**
2. **Always work on feature branches**
3. **Keep your fork's main in sync with upstream**
4. **Use descriptive branch names**: `feature/ollama-integration`, `fix/auth-bug`
5. **Rebase feature branches regularly** to avoid merge conflicts
6. **Delete merged branches** to keep your fork clean

## Remote Configuration
- `origin`: Your fork (jason-weddington/amazon-q-developer-cli.git)
- `upstream`: Original repo (aws/amazon-q-developer-cli.git)
