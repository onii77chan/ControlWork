class Person:
    def __init__(self, age=0):
        self._age = age

    def set_age(self, new_age):
        if new_age < 0:
            print(
                "Дорогой друг неужели ты прожил настолько долго что твй возраст аж пошел в обратную сторону?"
            )
        self._age = new_age

    def get_age(self):
        return self._age


p = Person()
p.set_age(25)
print(p.get_age())
p.set_age(-5)


class Animal:
    def __init__(self, name):
        self.name = name

    def speak(self):
        return "I am an animal"


class Dog(Animal):
    def __init__(self, name):
        super().__init__(name)

    def speak(self):
        return "Woof"


class Cat(Animal):
    def __init__(self, name):
        super().__init__(name)

    def speak(self):
        return "Meow"


dog = Dog("Buddy")
cat = Cat("Kitty")

print(dog.name, dog.speak())  # Вывод: Buddy Woof
print(cat.name, cat.speak())  # Вывод: Kitty Meow
